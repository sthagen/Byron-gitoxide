use bstr::ByteSlice;
use gix_actor::Identity;

#[test]
fn round_trip() -> gix_testtools::Result {
    static DEFAULTS: &[&[u8]] =     &[
        b"Sebastian Thiel <byronimo@gmail.com>",
        ".. ☺️Sebastian 王知明 Thiel🙌 .. <byronimo@gmail.com>".as_bytes(),
        b".. whitespace  \t  is explicitly allowed    - unicode aware trimming must be done elsewhere  <byronimo@gmail.com>"
    ];
    for input in DEFAULTS {
        let signature: Identity = gix_actor::IdentityRef::from_bytes::<()>(input).unwrap().into();
        let mut output = Vec::new();
        signature.write_to(&mut output)?;
        assert_eq!(output.as_bstr(), input.as_bstr());
    }
    Ok(())
}

#[test]
fn lenient_parsing() -> gix_testtools::Result {
    for input in [
        "First Last<<fl <First Last<fl@openoffice.org >> >",
        "First Last<fl <First Last<fl@openoffice.org>>\n",
    ] {
        let identity = gix_actor::IdentityRef::from_bytes::<()>(input.as_bytes()).unwrap();
        assert_eq!(identity.name, "First Last");
        assert_eq!(
            identity.email, "fl <First Last<fl@openoffice.org",
            "extra trailing and leading angled parens are stripped"
        );
        let signature: Identity = identity.into();
        let mut output = Vec::new();
        let err = signature.write_to(&mut output).unwrap_err();
        assert_eq!(
            err.to_string(),
            "Signature name or email must not contain '<', '>' or \\n",
            "this isn't roundtrippable as the name is technically incorrect - must not contain brackets"
        );
    }
    Ok(())
}
