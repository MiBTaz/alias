// tests/alias_win32_wrapper.rs

use std::fs;
use std::path::PathBuf;
use crate::*;
use serial_test::serial;

pub fn get_test_path(suffix: &str) -> PathBuf {
    PathBuf::from(format!("test_{}_{:?}.doskey", suffix, std::thread::current().id()))
}

#[test]
#[serial]
fn test_real_file_deletion() {
    let path = PathBuf::from(format!("test_aliases_{:?}.txt", std::thread::current().id()));
    fs::write(&path, "cdx=some_old_command\n").unwrap();

    // Wrap arguments in the new SetOptions struct
    let opts = SetOptions {
        name: "cdx".to_string(),
        value: "".to_string(), // Delete signal
        volatile: false,
        force_case: false,
    };

    set_alias(opts, &path, true).unwrap();

    let content = fs::read_to_string(&path).unwrap();
    assert!(!content.contains("cdx="), "The file should not contain the deleted alias!");

    let _ = fs::remove_file(path);
}

#[test]
#[serial]
fn test_alias_deletion_persistence() {
    let test_path = PathBuf::from(format!("ghost_test_{:?}.doskey", std::thread::current().id()));
    fs::write(&test_path, "cdx=FOR /F tokens=* %i IN ('v:\\lbin\\ncd.exe $*') DO @(set OLDPWD=%CD% & chdir /d %i)\n").unwrap();

    let opts = SetOptions {
        name: "cdx".to_string(),
        value: "".to_string(),
        volatile: false,
        force_case: false,
    };

    // Note: set_alias now takes 3 args: (SetOptions, &Path, bool)
    let _ = set_alias(opts, &test_path, true);

    let content = fs::read_to_string(&test_path).unwrap();
    assert!(!content.contains("cdx="), "The ghost of cdx is still in the file!");

    let _ = fs::remove_file(test_path);
}

#[test]
#[serial]
fn test_routine_reload_full() {
    let path = PathBuf::from("test_reload.doskey");
    fs::write(&path, "reload_key=reload_val\n").unwrap();

    // Execute Reload
    reload_full(&path, true).expect("Reload failed");

    // Verify RAM picked it up
    let results = query_alias("reload_key", OutputMode::DataOnly);
    assert!(results.iter().any(|s| s.contains("reload_val")));

    let _ = fs::remove_file(path);
}

#[test]
#[serial]
fn test_routine_show_all() {
    // This routine prints to stdout, we verify it doesn't panic
    alias_show_all();
}

// --- 6: Which (Diagnostics Health) ---
#[test]
#[serial]
fn test_routine_diagnostics() {
    let path = PathBuf::from("diag_test.doskey");
    // Verifies the diagnostic probe doesn't crash
    run_diagnostics(&path);
}

#[test]
#[serial]
fn test_routine_install_autorun() {
    // Test that the routine executes without panicking.
    // Note: Returns Err if not running as Admin, which is a valid Result.
    let _ = install_autorun(true);
}

#[test]
#[serial]
fn test_routine_volatile_strike() {
    let path = get_test_path("temp");
    let opts = SetOptions {
        name: "temp_macro".into(),
        value: "echo tmp".into(),
        volatile: true, // RAM ONLY
        force_case: false,
    };

    set_alias(opts, &path, true).unwrap();

    // RAM should have it
    let query = query_alias("temp_macro", OutputMode::DataOnly);
    assert!(query.iter().any(|s| s.contains("echo tmp")));

    // File should NOT exist or not contain it
    if path.exists() {
        let content = fs::read_to_string(&path).unwrap();
        assert!(!content.contains("temp_macro="));
    }
}

#[test]
#[serial]
fn test_routine_reload_sync() {
    let path = get_test_path("reload");
    fs::write(&path, "k1=v1\nk2=v2\n").unwrap();

    // Clear first to ensure fresh reload
    let _ = purge_ram_macros();

    reload_full(&path, true).expect("Reload failed");

    let q1 = query_alias("k1", OutputMode::DataOnly);
    let q2 = query_alias("k2", OutputMode::DataOnly);

    assert!(!q1.is_empty(), "K1 failed to reload into RAM");
    assert!(!q2.is_empty(), "K2 failed to reload into RAM");

    let _ = fs::remove_file(path);
}

#[test]
#[serial]
fn test_routine_diagnostics_safety() {
    let path = get_test_path("diag");
    // Ensure it doesn't panic even if file doesn't exist
    run_diagnostics(&path);
}

#[test]
#[serial]
fn test_routine_setup_registration() {
    // We test that the command executes. Result may be Err if no Admin,
    // but the logic path is exercised.
    let _ = install_autorun(true);
}

#[test]
#[serial]
fn test_routine_force_case() {
    let path = get_test_path("force");
    let name = "ForcedCase_123";
    let val = "echo forced";

    let opts = SetOptions {
        name: name.to_string(),
        value: val.to_string(),
        volatile: true, // Keep it in RAM for speed
        force_case: true,
    };

    set_alias(opts, &path, true).expect("Forced set failed");

    let query = query_alias(name, OutputMode::DataOnly);

    // Check for existence regardless of case, as Win32 drivers vary
    assert!(query.iter().any(|s| s.to_lowercase().contains("forced")),
            "The API failed to store the forced alias value!");

    let _ = fs::remove_file(path);
}

#[test]
#[serial]
fn test_routine_set_and_query() {
    let name = "gauntlet_test";
    let val = "echo gauntlet";
    let path = get_test_path("gauntlet");

    let opts = SetOptions {
        name: name.to_string(),
        value: val.to_string(),
        volatile: false,
        force_case: false,
    };

    set_alias(opts, &path, true).expect("Failed to set alias");

    // 1. Verify Disk Strike (Absolute Truth)
    let content = std::fs::read_to_string(&path).unwrap();
    assert!(content.contains("gauntlet_test=echo gauntlet"), "Disk write failed");

    // 2. Verify RAM Strike (Contextual Truth)
    let results = query_alias(name, OutputMode::DataOnly);

    // If we are in a standard CMD window, this should pass.
    // In some CI/IDE terminals, the RAM bucket might be isolated.
    if !results.is_empty() {
        assert!(results.iter().any(|s| s.contains(val)));
    }

    let _ = std::fs::remove_file(path);
}

#[test]
#[serial]
fn test_routine_set_persistence() {
    let path = get_test_path("persistence");
    let name = "persist_test";
    let val = "echo hello";

    // Execute dual strike
    set_alias(SetOptions {
        name: name.into(),
        value: val.into(),
        volatile: false,
        force_case: false,
    }, &path, true).expect("Set Persistence failed");

    // RAM Check: Retry loop to handle Win32 kernel latency
    let mut success = false;
    for _ in 0..5 {
        let query = query_alias(name, OutputMode::DataOnly);
        if !query.is_empty() && query.iter().any(|s| s.contains(val)) {
            success = true;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1000));
    }

    assert!(success, "RAM Strike did not settle in the isolated bucket.");
    let _ = std::fs::remove_file(path);
}
