use bstr::ByteSlice;
use git_features::progress;
use git_protocol::{handshake, FetchConnection};
use git_transport::Protocol;

use crate::fetch::{helper_unused, oid, transport, CloneDelegate, LsRemoteDelegate};

#[maybe_async::test(feature = "blocking-client", async(feature = "async-client", async_std::test))]
async fn clone() -> crate::Result {
    for with_keepalive in [false, true] {
        let out = Vec::new();
        let mut dlg = CloneDelegate::default();
        let fixture = format!(
            "v1/clone{}.response",
            with_keepalive.then(|| "-with-keepalive").unwrap_or_default()
        );
        git_protocol::fetch(
            transport(
                out,
                &fixture,
                Protocol::V1,
                git_transport::client::git::ConnectMode::Daemon,
            ),
            &mut dlg,
            helper_unused,
            progress::Discard,
            FetchConnection::TerminateOnSuccessfulCompletion,
            "agent",
        )
        .await?;
        assert_eq!(dlg.pack_bytes, 876, "{}: It be able to read pack bytes", fixture);
    }
    Ok(())
}

#[maybe_async::test(feature = "blocking-client", async(feature = "async-client", async_std::test))]
async fn ls_remote() -> crate::Result {
    let out = Vec::new();
    let mut delegate = LsRemoteDelegate::default();
    let mut transport = transport(
        out,
        "v1/clone.response",
        Protocol::V1,
        git_transport::client::git::ConnectMode::Daemon,
    );
    git_protocol::fetch(
        &mut transport,
        &mut delegate,
        helper_unused,
        progress::Discard,
        FetchConnection::AllowReuse,
        "agent",
    )
    .await?;

    assert_eq!(
        delegate.refs,
        vec![
            handshake::Ref::Symbolic {
                full_ref_name: "HEAD".into(),
                object: oid("808e50d724f604f69ab93c6da2919c014667bedb"),
                target: "refs/heads/master".into()
            },
            handshake::Ref::Direct {
                full_ref_name: "refs/heads/master".into(),
                object: oid("808e50d724f604f69ab93c6da2919c014667bedb")
            }
        ]
    );
    assert_eq!(
        transport.into_inner().1.as_bstr(),
        b"003agit-upload-pack does/not/matter\x00\x00value-only\x00key=value\x000000".as_bstr(),
        "we dont have to send anything in V1, except for the final flush byte to indicate we are done"
    );
    Ok(())
}

#[maybe_async::test(feature = "blocking-client", async(feature = "async-client", async_std::test))]
async fn ls_remote_handshake_failure_due_to_downgrade() -> crate::Result {
    let out = Vec::new();
    let delegate = LsRemoteDelegate::default();

    git_protocol::fetch(
        transport(
            out,
            "v1/clone.response",
            Protocol::V2,
            git_transport::client::git::ConnectMode::Process,
        ),
        delegate,
        helper_unused,
        progress::Discard,
        FetchConnection::AllowReuse,
        "agent",
    )
    .await
    .expect("V1 is OK for this transport");
    Ok(())
}
