use bstr::ByteSlice;
use git_features::progress;
use git_protocol::{fetch, FetchConnection};
use git_transport::Protocol;

use crate::fetch::{helper_unused, oid, transport, CloneDelegate, LsRemoteDelegate};

#[maybe_async::test(feature = "blocking-client", async(feature = "async-client", async_std::test))]
async fn clone() -> crate::Result {
    let out = Vec::new();
    let mut dlg = CloneDelegate::default();
    git_protocol::fetch(
        transport(
            out,
            "v1/clone.response",
            Protocol::V1,
            git_transport::client::git::ConnectMode::Daemon,
        ),
        &mut dlg,
        helper_unused,
        progress::Discard,
        FetchConnection::TerminateOnSuccessfulCompletion,
    )
    .await?;
    assert_eq!(dlg.pack_bytes, 876, "It be able to read pack bytes");
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
    )
    .await?;

    assert_eq!(
        delegate.refs,
        vec![
            fetch::Ref::Symbolic {
                path: "HEAD".into(),
                object: oid("808e50d724f604f69ab93c6da2919c014667bedb"),
                target: "refs/heads/master".into()
            },
            fetch::Ref::Direct {
                path: "refs/heads/master".into(),
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

    let err = match git_protocol::fetch(
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
    )
    .await
    {
        Ok(_) => panic!("the V1 is not allowed in this transport"),
        Err(err) => err,
    };
    assert_eq!(
        err.to_string(),
        "The transport didn't accept the advertised server version V1 and closed the connection client side"
    );
    Ok(())
}
