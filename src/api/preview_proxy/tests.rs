use super::*;

const SCRIPT: &str = r#"<script src="/__mutis/picker.js"></script>"#;

#[test]
fn injects_before_real_head_end_without_rewriting_attributes() {
    let head = r#"<HEAD><!-- </head> --><link href='/style.css' title="</head>"><script src=/main.js>"</head>"</script>"#;
    let tail = r#"</HEAD><form action="/submit?q=1">"#;
    let expected = format!("{head}{SCRIPT}{tail}");
    assert_eq!(
        html::inject(format!("{head}{tail}").as_bytes()),
        expected.as_bytes()
    );
}

#[test]
fn prepends_picker_and_preserves_comments_raw_text_and_non_utf8() {
    let html = b"<!-- </head><img src='/old'> --><script>const tag = \"<img src='/fake'>\";</script><p>\xe9";
    assert_eq!(html::inject(html), [SCRIPT.as_bytes(), html].concat());
}

#[test]
fn injects_listener_picker_path() {
    assert_eq!(PICKER_PATH, "/__mutis/picker.js");
    assert_eq!(html::inject(b""), SCRIPT.as_bytes());
}

#[test]
fn accepts_only_nonzero_decimal_ports() {
    for (input, expected) in [("1", 1), ("05173", 5173), ("65535", 65535)] {
        assert_eq!(validate_port(input).unwrap(), expected);
    }
    for input in [
        "", "0", "000", "65536", "+80", "-1", " 80", "80x", "%38", "８０",
    ] {
        assert_eq!(
            validate_port(input).unwrap_err().status,
            StatusCode::BAD_REQUEST
        );
    }
}

#[test]
fn rewrites_only_absolute_same_port_loopback_redirects() {
    let host: Authority = "127.0.0.1:49152".parse().unwrap();
    for location in [
        "http://127.0.0.1:5173/a?q=1#part",
        "http://[::1]:5173/a?q=1#part",
        "https://localhost:5173/a?q=1#part",
    ] {
        assert_eq!(
            rewrite_location(location, 5173, &host)
                .unwrap()
                .to_str()
                .unwrap(),
            "http://127.0.0.1:49152/a?q=1#part"
        );
    }
    for location in [
        "/a?q=1#part",
        "//localhost:5173/a",
        "http://example.test:5173/a",
        "http://localhost:5174/a",
        "relative",
    ] {
        assert!(rewrite_location(location, 5173, &host).is_none());
    }
}
