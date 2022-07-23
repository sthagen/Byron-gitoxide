#[cfg(feature = "blocking-client")]
use std::io::{BufRead, Write};
use std::{
    ops::Deref,
    sync::{Arc, Mutex},
};

use bstr::ByteSlice;
#[cfg(feature = "async-client")]
use futures_lite::{AsyncBufReadExt, AsyncWriteExt, StreamExt};
use git_transport::{
    client,
    client::{git, Transport, TransportV2Ext, TransportWithoutIO},
    Protocol, Service,
};

use crate::fixture_bytes;

#[maybe_async::test(feature = "blocking-client", async(feature = "async-client", async_std::test))]
async fn handshake_v1_and_request() -> crate::Result {
    let mut out = Vec::new();
    let server_response = fixture_bytes("v1/clone.response");
    let c = git::Connection::new(
        server_response.as_slice(),
        &mut out,
        Protocol::V1,
        "/foo.git",
        Some(("example.org", None)),
        git::ConnectMode::Daemon,
    );
    assert!(
        c.connection_persists_across_multiple_requests(),
        "tcp connections are stateful"
    );
    let c = c.custom_url(Some("anything".into()));
    assert_eq!(c.to_url(), "anything");
    let mut c = c.custom_url(None);
    assert_eq!(c.to_url(), "file:///foo.git");
    let mut res = c.handshake(Service::UploadPack, &[]).await?;
    assert_eq!(res.actual_protocol, Protocol::V1);
    assert_eq!(
        res.capabilities
            .iter()
            .map(|c| (c.name().to_owned(), c.value().map(ToOwned::to_owned)))
            .collect::<Vec<_>>(),
        [
            ("multi_ack", None),
            ("thin-pack", None),
            ("side-band", None),
            ("side-band-64k", None),
            ("ofs-delta", None),
            ("shallow", None),
            ("deepen-since", None),
            ("deepen-not", None),
            ("deepen-relative", None),
            ("no-progress", None),
            ("include-tag", None),
            ("multi_ack_detailed", None),
            ("symref", Some("HEAD:refs/heads/master")),
            ("object-format", Some("sha1")),
            ("agent", Some("git/2.28.0"))
        ]
        .iter()
        .map(|(n, v)| (
            n.as_bytes().as_bstr().to_owned(),
            v.map(|v| v.as_bytes().as_bstr().to_owned())
        ))
        .collect::<Vec<_>>()
    );
    let mut lines = res.refs.as_mut().expect("v1 protocol provides refs").lines();
    let mut refs = Vec::new();
    #[allow(clippy::while_let_on_iterator)] // needed in async version of test
    while let Some(line) = lines.next().await {
        refs.push(line?)
    }
    #[allow(clippy::drop_non_drop)] // needed for non-async version
    drop(lines);

    assert_eq!(
        refs,
        vec![
            "808e50d724f604f69ab93c6da2919c014667bedb HEAD",
            "808e50d724f604f69ab93c6da2919c014667bedb refs/heads/master"
        ]
    );
    drop(res);

    let writer = c.request(client::WriteMode::Binary, client::MessageKind::Flush)?;
    let nak_line = writer
        .into_read()
        .await?
        .lines()
        .next()
        .await
        .expect("exactly one line")?;
    assert_eq!(nak_line, "NAK");

    let mut writer = c.request(
        client::WriteMode::OneLfTerminatedLinePerWriteCall,
        client::MessageKind::Text(b"done"),
    )?;

    writer.write_all(b"hello").await?;
    writer.write_all(b"world").await?;

    let mut reader = writer.into_read().await?;
    let messages = Arc::new(Mutex::new(Vec::<String>::new()));
    reader.set_progress_handler(Some(Box::new({
        let sb = messages.clone();
        move |is_err, data| {
            assert!(!is_err);
            sb.deref()
                .lock()
                .expect("no poison")
                .push(std::str::from_utf8(data).expect("valid utf8").to_owned())
        }
    })));

    let expected_entries = 3;
    #[cfg(feature = "async-client")]
    let reader = futures_lite::io::BlockOn::new(reader);
    use git_pack::data::input;
    let entries = git_pack::data::input::BytesToEntriesIter::new_from_header(
        reader,
        input::Mode::Verify,
        input::EntryDataMode::Crc32,
        git_hash::Kind::Sha1,
    )?;
    assert_eq!(entries.count(), expected_entries);

    let sidebands = Arc::try_unwrap(messages)
        .expect("no other handle")
        .into_inner()
        .expect("no poison");
    assert_eq!(sidebands.len(), 6, "…along with some status messages");

    assert_eq!(
        out.as_slice().as_bstr(),
        b"002egit-upload-pack /foo.git\x00host=example.org\x000000000ahello\n\
    000aworld\n\
    0009done\n"
            .as_bstr(),
        "it sends the correct request"
    );
    Ok(())
}

#[maybe_async::test(feature = "blocking-client", async(feature = "async-client", async_std::test))]
async fn handshake_v1_process_mode() -> crate::Result {
    let mut out = Vec::new();
    let server_response = fixture_bytes("v1/clone.response");
    let mut c = git::Connection::new(
        server_response.as_slice(),
        &mut out,
        Protocol::V1,
        "/foo.git",
        Some(("example.org", None)),
        git::ConnectMode::Process,
    );
    c.handshake(Service::UploadPack, &[]).await?;

    assert_eq!(
        out.as_slice().as_bstr(),
        b"".as_bstr(),
        "it sends no introductory line to help the daemon start the right thing"
    );
    Ok(())
}

#[maybe_async::test(feature = "blocking-client", async(feature = "async-client", async_std::test))]
async fn handshake_v2_downgrade_to_v1() -> crate::Result {
    let mut out = Vec::new();
    let input = fixture_bytes("v1/clone.response");
    let mut c = git::Connection::new(
        input.as_slice(),
        &mut out,
        Protocol::V2,
        "/bar.git",
        Some(("example.org", None)),
        git::ConnectMode::Daemon,
    );
    let res = c.handshake(Service::UploadPack, &[]).await?;
    assert_eq!(res.actual_protocol, Protocol::V1);
    assert!(
        res.refs.is_some(),
        "V1 downgrades 'just happen', so we should have refs as part of the handshake"
    );
    drop(res);

    assert_eq!(c.supported_protocol_versions(), [Protocol::V2]);
    Ok(())
}

#[allow(clippy::unit_arg)] // side-effect of maybe-async
#[maybe_async::test(feature = "blocking-client", async(feature = "async-client", async_std::test))]
async fn handshake_v2_and_request() -> crate::Result {
    #[cfg(feature = "blocking-client")]
    return handshake_v2_and_request_inner().await;
    // This monstrosity simulates how one can process a pack received in async-io by transforming it into
    // blocking io::BufRead, while still handling the whole operation in a way that won't block the executor.
    // It's a way of `spawn_blocking()` in other executors. Currently this can only be done on a per-command basis.
    // Thinking about it, it's most certainly fine to do `fetch' commands on another thread and move the entire connection
    // there as it's always the end of an operation and a lot of IO is required that is blocking anyway, like accessing
    // commit graph information for fetch negotiations, and of course processing a received pack.
    #[cfg(feature = "async-client")]
    Ok(
        blocking::unblock(|| futures_lite::future::block_on(handshake_v2_and_request_inner()).expect("no failure"))
            .await,
    )
}

#[maybe_async::maybe_async]
async fn handshake_v2_and_request_inner() -> crate::Result {
    let mut out = Vec::new();
    let input = fixture_bytes("v2/clone.response");
    let mut c = git::Connection::new(
        input.as_slice(),
        &mut out,
        Protocol::V2,
        "/bar.git",
        Some(("example.org", None)),
        git::ConnectMode::Daemon,
    );
    assert!(
        c.connection_persists_across_multiple_requests(),
        "tcp connections are stateful despite the protocol version"
    );
    let res = c.handshake(Service::UploadPack, &[]).await?;
    assert_eq!(res.actual_protocol, Protocol::V2);
    assert!(
        res.refs.is_none(),
        "V2 needs a separate trip for getting refs (with additional capabilities)"
    );
    assert_eq!(
        res.capabilities
            .iter()
            .map(|c| (c.name().to_owned(), c.value().map(ToOwned::to_owned)))
            .collect::<Vec<_>>(),
        [
            ("agent", Some("git/2.28.0")),
            ("ls-refs", None),
            ("fetch", Some("shallow")),
            ("server-option", None),
            ("object-format", Some("sha1"))
        ]
        .iter()
        .map(|(k, v)| (k.as_bytes().into(), v.map(|v| v.as_bytes().into())))
        .collect::<Vec<_>>()
    );
    drop(res);

    let reader = c
        .invoke(
            "ls-refs",
            [("agent", Some("git/2.28.0")), ("object-format", Some("sha1"))]
                .iter()
                .cloned(),
            Some(
                [
                    "peel",
                    "symrefs",
                    "ref-prefix HEAD",
                    "ref-prefix refs/heads/",
                    "ref-prefix refs/tags",
                ]
                .iter()
                .map(|s| s.as_bytes().as_bstr().to_owned()),
            ),
        )
        .await?;

    let mut lines = reader.lines();
    let mut refs = Vec::new();
    #[allow(clippy::while_let_on_iterator)] // needed in async version of test
    while let Some(line) = lines.next().await {
        refs.push(line?)
    }
    assert_eq!(
        refs,
        vec![
            "808e50d724f604f69ab93c6da2919c014667bedb HEAD symref-target:refs/heads/master".to_string(),
            "808e50d724f604f69ab93c6da2919c014667bedb refs/heads/master".into()
        ]
    );
    drop(lines);

    let mut reader = c
        .invoke(
            "fetch",
            [
                ("agent", Some("git/2.28.0")),
                ("something-without-value", None),
                ("object-format", Some("sha1")),
            ]
            .iter()
            .cloned(),
            Some(
                [
                    "thin-pack",
                    "ofs-delta",
                    "want 808e50d724f604f69ab93c6da2919c014667bedb",
                    "done",
                ]
                .iter()
                .map(|s| s.as_bytes().as_bstr().to_owned()),
            ),
        )
        .await?;

    let mut line = String::new();
    reader.read_line(&mut line).await?;
    assert_eq!(line, "packfile\n");

    let messages = Arc::new(Mutex::new(Vec::<String>::new()));
    reader.set_progress_handler(Some(Box::new({
        let sb = messages.clone();
        move |is_err, data| {
            assert!(!is_err);
            sb.deref()
                .lock()
                .expect("no poison")
                .push(std::str::from_utf8(data).expect("valid utf8").to_owned())
        }
    })));

    let expected_entries = 3;
    #[cfg(feature = "async-client")]
    let reader = futures_lite::io::BlockOn::new(reader);

    use git_pack::data::input;
    let entries = git_pack::data::input::BytesToEntriesIter::new_from_header(
        reader,
        input::Mode::Verify,
        input::EntryDataMode::Crc32,
        git_hash::Kind::Sha1,
    )?;
    assert_eq!(entries.count(), expected_entries);

    let messages = Arc::try_unwrap(messages).expect("no other handle").into_inner()?;
    assert_eq!(messages.len(), 4);

    assert_eq!(
        out.as_slice().as_bstr(),
        b"0039git-upload-pack /bar.git\x00host=example.org\x00\x00version=2\x000014command=ls-refs
0015agent=git/2.28.0
0017object-format=sha1
00010009peel
000csymrefs
0014ref-prefix HEAD
001bref-prefix refs/heads/
0019ref-prefix refs/tags
00000012command=fetch
0015agent=git/2.28.0
001csomething-without-value
0017object-format=sha1
0001000ethin-pack
000eofs-delta
0032want 808e50d724f604f69ab93c6da2919c014667bedb
0009done
0000"
            .as_bstr(),
        "it sends the correct request, including the adjusted version"
    );
    Ok(())
}
