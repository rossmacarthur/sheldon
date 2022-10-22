use super::*;

#[test]
fn context_expand_tilde() {
    let ctx = Context {
        home: PathBuf::from("/test"),
        ..Default::default()
    };

    for (p, exp) in [
        ("/", "/"),
        ("/fol/der", "/fol/der"),
        ("~/", "/test"),
        ("~/fol/der", "/test/fol/der"),
    ] {
        assert_eq!(ctx.expand_tilde(PathBuf::from(p)), Path::new(exp));
    }
}

#[test]
fn context_replace_home() {
    let ctx = Context {
        home: PathBuf::from("/test"),
        ..Default::default()
    };

    for (p, exp) in [
        ("/not/home", "/not/home"),
        ("/test/home", "~/home"),
        ("/test/fol/der", "~/fol/der"),
    ] {
        assert_eq!(ctx.replace_home(p), Path::new(exp));
    }
}
