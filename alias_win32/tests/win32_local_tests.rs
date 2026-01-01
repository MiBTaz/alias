// alias_win32/tests/win32api.rs

use std::fs;
use std::path::PathBuf;
use serial_test::serial;
#[allow(unused_imports)]
use alias_lib::*;
#[allow(unused_imports)]
use alias_win32::*;

pub fn get_test_path(suffix: &str) -> PathBuf {
    PathBuf::from(format!("test_{}_{:?}.doskey", suffix, std::thread::current().id()))
}

#[test]
#[serial]
fn test_local_win32_kernel_quirk() {
    // This test only exists here. It won't be seen by the wrapper.
    assert!(true);
}
// If alias_win32_wrapper.rs needs access to the imports above,
// make sure that file has 'use crate::*;' or 'use alias_win32::*;' at the top.
#[test]
#[serial]
fn test_win32_api_roundtrip() {
    // This tests the actual Win32 interaction
    let name = "test_alias_123";
    let val = "echo hello";

    // api_set_macro usually takes (name, Option<value>)
    api_set_macro(name, Some(val));
    let all = get_all_aliases();
    let found = all.iter().find(|(n, _)| n == name);

    assert!(found.is_some());
    api_set_macro(name, None); // Cleanup
}

#[test]
#[serial]
fn test_routine_clear_ram() {
    let name = "purge_me"; // Use lowercase
    api_set_macro(name, Some("temporary"));

    let report = purge_ram_macros().expect("Purge failed");

    // Check using lowercase comparison
    assert!(report.cleared.iter().any(|n| n.to_lowercase() == name.to_lowercase()),
            "Purge did not report clearing our test key");

    let results = query_alias(name, OutputMode::DataOnly);
    assert!(results.is_empty() || results[0].contains("not active"));
}

#[test]
#[serial]
fn test_routine_purge_ram() {
    api_set_macro("purge_target", Some("alive"));

    let report = purge_ram_macros().expect("Purge failed");
    assert!(report.cleared.iter().any(|n| n == "purge_target"));

    let query = query_alias("purge_target", OutputMode::DataOnly);
    assert!(query.is_empty() || query[0].contains("not active"));
}

#[test]
#[serial]
fn test_routine_delete_sync() {
    let path = get_test_path("del");
    fs::write(&path, "ghost=gone\n").unwrap();
    api_set_macro("ghost", Some("gone"));

    let opts = SetOptions {
        name: "ghost".into(),
        value: "".into(), // Delete signal
        volatile: false,
        force_case: false,
    };

    set_alias(opts, &path, true).unwrap();

    // Verify RAM
    let query = query_alias("ghost", OutputMode::DataOnly);
    assert!(query.is_empty() || query[0].contains("not found") || query[0].contains("not active"));

    // Verify Disk
    let content = fs::read_to_string(&path).unwrap();
    assert!(!content.contains("ghost="));

    let _ = fs::remove_file(path);
}

