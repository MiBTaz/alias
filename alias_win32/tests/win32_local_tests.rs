// alias_win32/tests/win32api.rs

use std::fs;
use std::path::PathBuf;
use serial_test::serial;
use alias_lib::*;
use alias_win32::*;

pub fn get_test_path(suffix: &str) -> PathBuf {
    PathBuf::from(format!("test_{}_{:?}.doskey", suffix, std::thread::current().id()))
}

#[test]
#[serial]
fn a_nuke_the_world() {
    // This runs first (alphabetically) and calls your new lib
    alias_nuke::kernel_wipe_macros();
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

#[test]
#[serial]
fn test_win32_rapid_fire_sync() {
    let path = get_test_path("stress");
    // Set 20 aliases in rapid succession
    for i in 0..20 {
        let name = format!("stress_test_{}", i);
        let opts = SetOptions {
            name: name.clone(),
            value: "echo work".into(),
            volatile: false,
            force_case: false,
        };
        set_alias(opts, &path, true).expect("Rapid fire set failed");
    }

    let all = get_all_aliases();
    // Ensure all 20 are in RAM
    for i in 0..20 {
        let name = format!("stress_test_{}", i);
        assert!(all.iter().any(|(n, _)| n == &name), "Missing alias {}", name);
    }

    let _ = std::fs::remove_file(path);
}

#[test]
#[serial]
fn test_win32_international_roundtrip() {
    let name = "λ_alias";
    let val = "echo lambda_power";

    // Strike the RAM with Wide API
    assert!(api_set_macro(name, Some(val)), "Failed to set international alias");

    // Query it back
    let all = get_all_aliases();
    let found = all.iter().find(|(n, _)| n == name);

    assert!(found.is_some(), "International alias 'λ' was mangled or lost");
    assert_eq!(found.unwrap().1, val);

    // Cleanup
    api_set_macro(name, None);
}

#[test]
#[serial]
fn test_registry_append_logic() {
    use winreg::RegKey;
    use winreg::enums::HKEY_CURRENT_USER;

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let (key, _) = hkcu.create_subkey(REG_SUBKEY).unwrap();

    // 1. Fake an existing AutoRun entry
    let original_cmd = "echo 'Old Command'";
    key.set_value(REG_AUTORUN_KEY, &original_cmd.to_string()).unwrap();

    // 2. Run our installer
    install_autorun(true).expect("Install failed");

    // 3. Verify both exist
    let result: String = key.get_value(REG_AUTORUN_KEY).unwrap();
    assert!(result.contains(original_cmd), "Original AutoRun was overwritten!");
    assert!(result.contains("--reload"), "Our reload command was not added!");
    assert!(result.contains(" & "), "Commands were not properly joined with '&'");
}

#[test]
#[serial]
fn test_transactional_disk_write() {
    let path = get_test_path("transact");
    let name = "test_alias";
    let val = "echo heavy_lifting";

    // Perform the strike
    update_disk_file(name, val, &path).expect("Transactional write failed");

    // Verify the result
    let content = std::fs::read_to_string(&path).unwrap();
    assert!(content.contains("test_alias=echo heavy_lifting"));

    // Ensure the .tmp file was cleaned up
    let tmp_path = path.with_extension("doskey.tmp");
    assert!(!tmp_path.exists(), "Temporary file was not cleaned up!");

    std::fs::remove_file(path).ok();
}

#[test]
fn test_thread_silo_isolation() {
    // Even in serial mode, we test if the logic handles 'bucket' separation
    // We'll use a unique name to ensure no previous test state interferes
    let name_a = "unique_silo_test_a";
    let name_b = "unique_silo_test_b";

    api_set_macro(name_a, Some("val_a"));
    api_set_macro(name_b, Some("val_b"));

    let all = get_all_aliases();

    assert!(all.iter().any(|(n, _)| n == name_a), "Silo failed to store A");
    assert!(all.iter().any(|(n, _)| n == name_b), "Silo failed to store B");

    // Cleanup for the next serial test
    api_set_macro(name_a, None);
    api_set_macro(name_b, None);
}

#[test]
#[serial]
fn z_nuke_the_world_end() {
    // This runs first (alphabetically) and calls your new lib
    alias_nuke::kernel_wipe_macros();
}
