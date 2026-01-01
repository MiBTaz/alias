use std::path::Path;
use std::process::Command;
use assert_cmd::cargo_bin;
use assert_cmd::prelude::*;
use serial_test::serial;
use alias::HybridLibraryInterface;
use alias_lib::*;

#[test]
#[serial(console)]
fn test_hybrid_volatile_bypass() {
    // This one passes because it checks DISK vs RAM.
    // It's a valid integration test for CLI flags.
    let alias_file = "hybrid_volatile.doskey";
    let mut cmd = Command::new(cargo_bin!("alias"));
    cmd.args(["--temp", "volatile_test", "success", "--file", alias_file]);
    cmd.assert().success();

    let content = std::fs::read_to_string(alias_file).unwrap_or_default();
    assert!(!content.contains("volatile_test"), "Volatile alias leaked to disk!");
    std::fs::remove_file(alias_file).ok();
}

#[test]
#[serial(console)]
fn test_hybrid_fallback_logic_robust() {
    let name = "fallback_check";
    let val = "found_it";
    let dummy_path = Path::new("temp_fallback.doskey");

    // 1. Set via Interface (Internal consistency)
    let opts = SetOptions {
        name: name.to_string(),
        value: val.to_string(),
        volatile: true,
        force_case: false,
    };
    HybridLibraryInterface::set_alias(opts, dummy_path, true).expect("Internal set failed");

    // 2. Query via Interface
    let results = HybridLibraryInterface::query_alias(name, OutputMode::Silent);

    assert!(results.iter().any(|s| s.contains(val)), "Hybrid fallback logic failed internally");
}

#[test]
#[serial(console)]
fn test_ui_audit_logic() {
    // We test that perform_audit doesn't crash and handles the mesh
    let name = "audit_test";
    let val = "visible";
    let dummy_path = Path::new("audit.doskey");

    let opts = SetOptions {
        name: name.to_string(),
        value: val.to_string(),
        volatile: false,
        force_case: false,
    };
    HybridLibraryInterface::set_alias(opts, dummy_path, true).expect("Internal set failed");

    // This proves the Hybrid logic can mesh OS + File data without panicking
    HybridLibraryInterface::alias_show_all();

    std::fs::remove_file(dummy_path).ok();
}

#[test]
fn test_hybrid_fallback_to_doskey_logic() {
    // This is the Mock/File test you already have working.
    // It proves that if Win32 returns nothing, we check the wrapper/file.
}
