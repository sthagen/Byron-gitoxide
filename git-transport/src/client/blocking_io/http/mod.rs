use std::{
    borrow::Cow,
    convert::Infallible,
    io::{BufRead, Read},
};

use git_packetline::PacketLineRef;
pub use traits::{Error, GetResponse, Http, PostResponse};

use crate::{
    client::{self, capabilities, Capabilities, ExtendedBufRead, HandleProgress, MessageKind, RequestWriter},
    Protocol, Service,
};

#[cfg(feature = "http-client-curl")]
mod curl;

///
mod traits;

/// The actual http client implementation.
#[cfg(feature = "http-client-curl")]
pub type Impl = curl::Curl;

/// A transport for supporting arbitrary http clients by abstracting interactions with them into the [Http] trait.
pub struct Transport<H: Http> {
    url: String,
    user_agent_header: &'static str,
    desired_version: Protocol,
    supported_versions: [Protocol; 1],
    actual_version: Protocol,
    http: H,
    service: Option<Service>,
    line_provider: Option<git_packetline::StreamingPeekableIter<H::ResponseBody>>,
    identity: Option<git_sec::identity::Account>,
}

impl<H: Http> Transport<H> {
    /// Create a new instance with `http` as implementation to communicate to `url` using the given `desired_version` of the `git` protocol.
    pub fn new_http(http: H, url: &str, desired_version: Protocol) -> Self {
        Transport {
            url: url.to_owned(),
            user_agent_header: concat!("User-Agent: git/oxide-", env!("CARGO_PKG_VERSION")),
            desired_version,
            actual_version: desired_version,
            supported_versions: [desired_version],
            service: None,
            http,
            line_provider: None,
            identity: None,
        }
    }
}

#[cfg(feature = "http-client-curl")]
impl Transport<Impl> {
    /// Create a new instance to communicate to `url` using the given `desired_version` of the `git` protocol.
    ///
    /// Note that the actual implementation depends on feature toggles.
    pub fn new(url: &str, desired_version: Protocol) -> Self {
        Self::new_http(Impl::default(), url, desired_version)
    }
}

impl<H: Http> Transport<H> {
    fn check_content_type(service: Service, kind: &str, headers: <H as Http>::Headers) -> Result<(), client::Error> {
        let wanted_content_type = format!("content-type: application/x-{}-{}", service.as_str(), kind);
        if !headers
            .lines()
            .collect::<Result<Vec<_>, _>>()?
            .iter()
            .any(|l| l.to_lowercase() == wanted_content_type)
        {
            return Err(client::Error::Http(Error::Detail {
                description: format!(
                    "Didn't find '{}' header to indicate 'smart' protocol, and 'dumb' protocol is not supported.",
                    wanted_content_type
                ),
            }));
        }
        Ok(())
    }

    #[allow(clippy::unnecessary_wraps, unknown_lints)]
    fn add_basic_auth_if_present(&self, headers: &mut Vec<Cow<'_, str>>) -> Result<(), client::Error> {
        if let Some(git_sec::identity::Account { username, password }) = &self.identity {
            #[cfg(not(debug_assertions))]
            if self.url.starts_with("http://") {
                return Err(client::Error::AuthenticationRefused(
                    "Will not send credentials in clear text over http",
                ));
            }
            headers.push(Cow::Owned(format!(
                "Authorization: Basic {}",
                base64::encode(format!("{}:{}", username, password))
            )))
        }
        Ok(())
    }
}

fn append_url(base: &str, suffix: &str) -> String {
    let mut buf = base.to_owned();
    if base.as_bytes().last() != Some(&b'/') {
        buf.push('/');
    }
    buf.push_str(suffix);
    buf
}

impl<H: Http> client::TransportWithoutIO for Transport<H> {
    fn set_identity(&mut self, identity: git_sec::identity::Account) -> Result<(), client::Error> {
        self.identity = Some(identity);
        Ok(())
    }

    fn request(
        &mut self,
        write_mode: client::WriteMode,
        on_into_read: MessageKind,
    ) -> Result<RequestWriter<'_>, client::Error> {
        let service = self.service.expect("handshake() must have been called first");
        let url = append_url(&self.url, service.as_str());
        let static_headers = &[
            Cow::Borrowed(self.user_agent_header),
            Cow::Owned(format!("Content-Type: application/x-{}-request", service.as_str())),
            format!("Accept: application/x-{}-result", service.as_str()).into(),
            "Expect:".into(), // needed to avoid sending Expect: 100-continue, which adds another response and only CURL wants that
        ];
        let mut dynamic_headers = Vec::new();
        self.add_basic_auth_if_present(&mut dynamic_headers)?;
        if self.actual_version != Protocol::V1 {
            dynamic_headers.push(Cow::Owned(format!(
                "Git-Protocol: version={}",
                self.actual_version as usize
            )));
        }

        let PostResponse {
            headers,
            body,
            post_body,
        } = self.http.post(&url, static_headers.iter().chain(&dynamic_headers))?;
        let line_provider = self
            .line_provider
            .as_mut()
            .expect("handshake to have been called first");
        line_provider.replace(body);
        Ok(RequestWriter::new_from_bufread(
            post_body,
            Box::new(HeadersThenBody::<H, _> {
                service,
                headers: Some(headers),
                body: line_provider.as_read_without_sidebands(),
            }),
            write_mode,
            on_into_read,
        ))
    }

    fn to_url(&self) -> String {
        self.url.clone()
    }

    fn supported_protocol_versions(&self) -> &[Protocol] {
        &self.supported_versions
    }

    fn connection_persists_across_multiple_requests(&self) -> bool {
        false
    }
}

impl<H: Http> client::Transport for Transport<H> {
    fn handshake<'a>(
        &mut self,
        service: Service,
        extra_parameters: &'a [(&'a str, Option<&'a str>)],
    ) -> Result<client::SetServiceResponse<'_>, client::Error> {
        let url = append_url(self.url.as_ref(), &format!("info/refs?service={}", service.as_str()));
        let static_headers = [Cow::Borrowed(self.user_agent_header)];
        let mut dynamic_headers = Vec::<Cow<'_, str>>::new();
        if self.desired_version != Protocol::V1 || !extra_parameters.is_empty() {
            let mut parameters = if self.desired_version != Protocol::V1 {
                let mut p = format!("version={}", self.desired_version as usize);
                if !extra_parameters.is_empty() {
                    p.push(':');
                }
                p
            } else {
                String::new()
            };
            parameters.push_str(
                &extra_parameters
                    .iter()
                    .map(|(key, value)| match value {
                        Some(value) => format!("{}={}", key, value),
                        None => key.to_string(),
                    })
                    .collect::<Vec<_>>()
                    .join(":"),
            );
            dynamic_headers.push(format!("Git-Protocol: {}", parameters).into());
        }
        self.add_basic_auth_if_present(&mut dynamic_headers)?;
        let GetResponse { headers, body } = self
            .http
            .get(url.as_ref(), static_headers.iter().chain(&dynamic_headers))?;
        <Transport<H>>::check_content_type(service, "advertisement", headers)?;

        let line_reader = self
            .line_provider
            .get_or_insert_with(|| git_packetline::StreamingPeekableIter::new(body, &[PacketLineRef::Flush]));

        let mut announced_service = String::new();
        line_reader.as_read().read_to_string(&mut announced_service)?;
        let expected_service_announcement = format!("# service={}", service.as_str());
        if announced_service.trim() != expected_service_announcement {
            return Err(client::Error::Http(Error::Detail {
                description: format!(
                    "Expected to see {:?}, but got {:?}",
                    expected_service_announcement,
                    announced_service.trim()
                ),
            }));
        }

        let capabilities::recv::Outcome {
            capabilities,
            refs,
            protocol: actual_protocol,
        } = Capabilities::from_lines_with_version_detection(line_reader)?;
        self.actual_version = actual_protocol;
        self.service = Some(service);
        Ok(client::SetServiceResponse {
            actual_protocol,
            capabilities,
            refs,
        })
    }
}

struct HeadersThenBody<H: Http, B: Unpin> {
    service: Service,
    headers: Option<H::Headers>,
    body: B,
}

impl<H: Http, B: Unpin> HeadersThenBody<H, B> {
    fn handle_headers(&mut self) -> std::io::Result<()> {
        if let Some(headers) = self.headers.take() {
            <Transport<H>>::check_content_type(self.service, "result", headers)
                .map_err(|err| std::io::Error::new(std::io::ErrorKind::Other, err))?
        }
        Ok(())
    }
}

impl<H: Http, B: ExtendedBufRead + Unpin> Read for HeadersThenBody<H, B> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.handle_headers()?;
        self.body.read(buf)
    }
}

impl<H: Http, B: ExtendedBufRead + Unpin> BufRead for HeadersThenBody<H, B> {
    fn fill_buf(&mut self) -> std::io::Result<&[u8]> {
        self.handle_headers()?;
        self.body.fill_buf()
    }

    fn consume(&mut self, amt: usize) {
        self.body.consume(amt)
    }
}

impl<H: Http, B: ExtendedBufRead + Unpin> ExtendedBufRead for HeadersThenBody<H, B> {
    fn set_progress_handler(&mut self, handle_progress: Option<HandleProgress>) {
        self.body.set_progress_handler(handle_progress)
    }

    fn peek_data_line(&mut self) -> Option<std::io::Result<Result<&[u8], client::Error>>> {
        if let Err(err) = self.handle_headers() {
            return Some(Err(err));
        }
        self.body.peek_data_line()
    }

    fn reset(&mut self, version: Protocol) {
        self.body.reset(version)
    }

    fn stopped_at(&self) -> Option<MessageKind> {
        self.body.stopped_at()
    }
}

/// Connect to the given `url` via HTTP/S using the `desired_version` of the `git` protocol, with `http` as implementation.
#[cfg(all(feature = "http-client", not(feature = "http-client-curl")))]
pub fn connect_http<H: Http>(http: H, url: &str, desired_version: Protocol) -> Result<Transport<H>, Infallible> {
    Ok(Transport::new_http(http, url, desired_version))
}

/// Connect to the given `url` via HTTP/S using the `desired_version` of the `git` protocol.
#[cfg(feature = "http-client-curl")]
pub fn connect(url: &str, desired_version: Protocol) -> Result<Transport<Impl>, Infallible> {
    Ok(Transport::new(url, desired_version))
}
