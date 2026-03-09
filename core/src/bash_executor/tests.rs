use super::*;

#[test]
fn test_simple_exec() {
    let result = execute_bash("echo hello", &BashExecConfig::default());
    assert!(result.is_ok());
    let res = result.unwrap();
    assert!(res.success);
    assert!(res.stdout.contains("hello"));
}

#[test]
fn test_process_registry() {
    register_process("test1", 12345, "sleep 100");
    let info = get_process("test1");
    assert!(info.is_some());
    assert_eq!(info.unwrap().pid, 12345);
}

#[test]
fn test_auto_approval_only_in_test_mode() {
    assert!(exec::should_allow_test_auto_approval_for_tests(true, true));
    assert!(!exec::should_allow_test_auto_approval_for_tests(
        true, false
    ));
    assert!(!exec::should_allow_test_auto_approval_for_tests(
        false, true
    ));
    assert!(!exec::should_allow_test_auto_approval_for_tests(
        false, false
    ));
}
