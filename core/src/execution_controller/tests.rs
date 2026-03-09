use super::*;

#[cfg(target_os = "macos")]
#[test]
fn parse_idle_ns_from_decimal_line() {
    let sample = r#"    | | |   "HIDIdleTime" = 31574920250"#;
    let parsed = parse_idle_ns_from_ioreg(sample).expect("decimal HIDIdleTime should parse");
    assert_eq!(parsed, 31_574_920_250);
}

#[cfg(target_os = "macos")]
#[test]
fn parse_idle_ns_from_hex_line() {
    let sample = r#"    | | |   "HIDIdleTime" = 0x75BCD15"#;
    let parsed = parse_idle_ns_from_ioreg(sample).expect("hex HIDIdleTime should parse");
    assert_eq!(parsed, 123_456_789);
}
