use super::*;

#[test]
fn test_exec_allowlist_pattern_validation_defaults_secure() {
    assert!(validate_exec_allowlist_pattern_with_flags("*", false, false).is_err());
    assert!(validate_exec_allowlist_pattern_with_flags("all", false, false).is_err());
    assert!(validate_exec_allowlist_pattern_with_flags("re:^ls", false, false).is_err());
    assert!(validate_exec_allowlist_pattern_with_flags("/^ls/", false, false).is_err());
    assert!(validate_exec_allowlist_pattern_with_flags("ls -la", false, false).is_ok());
    assert!(validate_exec_allowlist_pattern_with_flags("git*", false, false).is_ok());
}

#[test]
fn test_exec_allowlist_pattern_match_with_flags() {
    assert!(!exec_pattern_match_with_flags(
        "*", "rm -rf /", false, false
    ));
    assert!(exec_pattern_match_with_flags(
        "*",
        "echo hello",
        true,
        false
    ));
    assert!(!exec_pattern_match_with_flags(
        "re:^ls\\b",
        "ls -la",
        false,
        false
    ));
    assert!(exec_pattern_match_with_flags(
        "re:^ls\\b",
        "ls -la",
        false,
        true
    ));
    assert!(exec_pattern_match_with_flags(
        "git*",
        "git status",
        false,
        false
    ));
    assert!(!exec_pattern_match_with_flags(
        "git*", "ls -la", false, false
    ));
}
