use git_testtools::hex_to_id as oid;
use git_transport::{client, client::Capabilities};

use crate::fetch::{refs, refs::shared::InternalRef, Ref};

#[maybe_async::test(feature = "blocking-client", async(feature = "async-client", async_std::test))]
async fn extract_references_from_v2_refs() {
    let input = &mut "808e50d724f604f69ab93c6da2919c014667bedb HEAD symref-target:refs/heads/main
808e50d724f604f69ab93c6da2919c014667bedb MISSING_NAMESPACE_TARGET symref-target:(null)
unborn HEAD symref-target:refs/heads/main
unborn refs/heads/symbolic symref-target:refs/heads/target
808e50d724f604f69ab93c6da2919c014667bedb refs/heads/main
7fe1b98b39423b71e14217aa299a03b7c937d656 refs/tags/foo peeled:808e50d724f604f69ab93c6da2919c014667bedb
7fe1b98b39423b71e14217aa299a03b7c937d6ff refs/tags/blaz
"
    .as_bytes();

    let out = refs::from_v2_refs(input).await.expect("no failure on valid input");

    assert_eq!(
        out,
        vec![
            Ref::Symbolic {
                full_ref_name: "HEAD".into(),
                target: "refs/heads/main".into(),
                object: oid("808e50d724f604f69ab93c6da2919c014667bedb")
            },
            Ref::Direct {
                full_ref_name: "MISSING_NAMESPACE_TARGET".into(),
                object: oid("808e50d724f604f69ab93c6da2919c014667bedb")
            },
            Ref::Unborn {
                full_ref_name: "HEAD".into(),
                target: "refs/heads/main".into(),
            },
            Ref::Unborn {
                full_ref_name: "refs/heads/symbolic".into(),
                target: "refs/heads/target".into(),
            },
            Ref::Direct {
                full_ref_name: "refs/heads/main".into(),
                object: oid("808e50d724f604f69ab93c6da2919c014667bedb")
            },
            Ref::Peeled {
                full_ref_name: "refs/tags/foo".into(),
                tag: oid("7fe1b98b39423b71e14217aa299a03b7c937d656"),
                object: oid("808e50d724f604f69ab93c6da2919c014667bedb")
            },
            Ref::Direct {
                full_ref_name: "refs/tags/blaz".into(),
                object: oid("7fe1b98b39423b71e14217aa299a03b7c937d6ff")
            },
        ]
    )
}

#[maybe_async::test(feature = "blocking-client", async(feature = "async-client", async_std::test))]
async fn extract_references_from_v1_refs() {
    let input = &mut "73a6868963993a3328e7d8fe94e5a6ac5078a944 HEAD
21c9b7500cb144b3169a6537961ec2b9e865be81 MISSING_NAMESPACE_TARGET
73a6868963993a3328e7d8fe94e5a6ac5078a944 refs/heads/main
8e472f9ccc7d745927426cbb2d9d077de545aa4e refs/pull/13/head
dce0ea858eef7ff61ad345cc5cdac62203fb3c10 refs/tags/git-commitgraph-v0.0.0
21c9b7500cb144b3169a6537961ec2b9e865be81 refs/tags/git-commitgraph-v0.0.0^{}"
        .as_bytes();
    let out = refs::from_v1_refs_received_as_part_of_handshake_and_capabilities(
        input,
        Capabilities::from_bytes(b"\0symref=HEAD:refs/heads/main symref=MISSING_NAMESPACE_TARGET:(null)")
            .expect("valid capabilities")
            .0
            .iter(),
    )
    .await
    .expect("no failure from valid input");
    assert_eq!(
        out,
        vec![
            Ref::Symbolic {
                full_ref_name: "HEAD".into(),
                target: "refs/heads/main".into(),
                object: oid("73a6868963993a3328e7d8fe94e5a6ac5078a944")
            },
            Ref::Direct {
                full_ref_name: "MISSING_NAMESPACE_TARGET".into(),
                object: oid("21c9b7500cb144b3169a6537961ec2b9e865be81")
            },
            Ref::Direct {
                full_ref_name: "refs/heads/main".into(),
                object: oid("73a6868963993a3328e7d8fe94e5a6ac5078a944")
            },
            Ref::Direct {
                full_ref_name: "refs/pull/13/head".into(),
                object: oid("8e472f9ccc7d745927426cbb2d9d077de545aa4e")
            },
            Ref::Peeled {
                full_ref_name: "refs/tags/git-commitgraph-v0.0.0".into(),
                tag: oid("dce0ea858eef7ff61ad345cc5cdac62203fb3c10"),
                object: oid("21c9b7500cb144b3169a6537961ec2b9e865be81")
            },
        ]
    )
}

#[test]
fn extract_symbolic_references_from_capabilities() -> Result<(), client::Error> {
    let caps = client::Capabilities::from_bytes(
        b"\0unrelated symref=HEAD:refs/heads/main symref=ANOTHER:refs/heads/foo symref=MISSING_NAMESPACE_TARGET:(null) agent=git/2.28.0",
    )?
    .0;
    let out = refs::shared::from_capabilities(caps.iter()).expect("a working example");

    assert_eq!(
        out,
        vec![
            InternalRef::SymbolicForLookup {
                path: "HEAD".into(),
                target: Some("refs/heads/main".into())
            },
            InternalRef::SymbolicForLookup {
                path: "ANOTHER".into(),
                target: Some("refs/heads/foo".into())
            },
            InternalRef::SymbolicForLookup {
                path: "MISSING_NAMESPACE_TARGET".into(),
                target: None
            }
        ]
    );
    Ok(())
}
