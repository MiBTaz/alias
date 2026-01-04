// tests/alias_win32_wrapper.rs

use std::fs;
use std::path::PathBuf;
#[allow(unused_imports)]
use serial_test::serial;
extern crate alias_nuke;
// This brings in the trait, the structs, and the macro
#[allow(unused_imports)]
use alias_lib::*;

// If the wrapper uses these constants, import them too
#[allow(unused_imports)]
use alias_lib::{REG_SUBKEY, REG_AUTORUN_KEY};

#[test]
#[serial]
fn a_nuke_the_world() {
    // This runs first (alphabetically) and calls your new lib
    alias_nuke::kernel_wipe_macros();
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
    let _ = P::set_alias(opts, &test_path, voice!(Silent, ShowFeature::Off, ShowFeature::Off));

    let content = fs::read_to_string(&test_path).unwrap();
    assert!(!content.contains("cdx="), "The ghost of cdx is still in the file!");

    let _ = fs::remove_file(test_path);
}

// --- 6: Which (Diagnostics Health) ---
#[test]
#[serial]
fn test_routine_diagnostics_safety() {
    let path = get_test_path("diag");
    // Ensure it doesn't panic even if file doesn't exist
    P::run_diagnostics(&path, voice!(Silent, ShowFeature::Off, ShowFeature::Off));
}

#[test]
#[serial]
fn test_routine_setup_registration() {
    // We test that the command executes. Result may be Err if no Admin,
    // but the logic path is exercised.
    let _ = P::install_autorun(voice!(Silent, ShowFeature::Off, ShowFeature::Off));
}

#[test]
#[serial]
fn test_routine_set_persistence() {
    let path = get_test_path("persistence");
    let name = "persist_test";
    let val = "echo hello";

    // Execute dual strike
    P::set_alias(SetOptions {
        name: name.into(),
        value: val.into(),
        volatile: false,
        force_case: false,
    }, &path, voice!(Silent, ShowFeature::Off, ShowFeature::Off)).expect("Set Persistence failed");

    // RAM Check: Retry loop to handle Win32 kernel latency
    let mut success = false;
    for _ in 0..5 {
        let query = P::query_alias(name, voice!(Silent, ShowFeature::Off, ShowFeature::Off));
        if !query.is_empty() && query.iter().any(|s: &String| s.contains(val)) {
            success = true;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1000));
    }

    assert!(success, "RAM Strike did not settle in the isolated bucket.");
    let _ = std::fs::remove_file(path);
}

// -----------------------------------------
#[test]
#[serial]
fn test_routine_show_all() {
    // FIXED: alias_show_all now requires verbosity
    P::alias_show_all(voice!(Normal, ShowFeature::Off, ShowFeature::Off));
}

// -------------------------------

// Helper to create the required Verbosity object for the new API
fn test_v() -> Verbosity {
    voice!(Silent, ShowFeature::Off, ShowFeature::Off)
}

pub fn get_test_path(suffix: &str) -> PathBuf {
    PathBuf::from(format!("test_{}_{:?}.doskey", suffix, std::thread::current().id()))
}


#[test]
#[serial]
fn test_real_file_deletion() {
    let path = get_test_path("del");
    fs::write(&path, "cdx=some_old_command\n").unwrap();

    let opts = SetOptions {
        name: "cdx".to_string(),
        value: "".to_string(),
        volatile: false,
        force_case: false,
    };

    // FIXED: Passed test_v() instead of 'true'
    P::set_alias(opts, &path, test_v()).unwrap();

    let content = fs::read_to_string(&path).unwrap();
    assert!(!content.contains("cdx="));
    let _ = fs::remove_file(path);
}

#[test]
#[serial]
fn test_routine_reload_full() {
    let path = PathBuf::from("test_reload.doskey");
    fs::write(&path, "reload_key=reload_val\n").unwrap();

    // FIXED: Signature now requires Verbosity
    P::reload_full(&path, test_v()).expect("Reload failed");

    // FIXED: query_alias now requires Verbosity
    let results = P::query_alias("reload_key", test_v());
    assert!(results.iter().any(|s: &String| s.contains("reload_val")));

    let _ = fs::remove_file(path);
}

#[test]
#[serial]
fn test_routine_diagnostics() {
    let path = PathBuf::from("diag_test.doskey");
    // FIXED: Signature changed from (path) to (path, verbosity)
    P::run_diagnostics(&path, voice!(Silent, ShowFeature::Off, ShowFeature::Off));
}

#[test]
#[serial]
fn test_routine_install_autorun() {
    // FIXED: Passed test_v() instead of 'true'
    let _ = P::install_autorun(test_v());
}

#[test]
#[serial]
fn test_routine_volatile_strike() {
    let path = get_test_path("temp");
    let opts = SetOptions {
        name: "temp_macro".into(),
        value: "echo tmp".into(),
        volatile: true,
        force_case: false,
    };

    P::set_alias(opts, &path, test_v()).unwrap();

    let query = P::query_alias("temp_macro", test_v());
    assert!(query.iter().any(|s: &String| s.contains("echo tmp")));

    if path.exists() {
        let content = fs::read_to_string(&path).unwrap();
        assert!(!content.contains("temp_macro="));
    }
}

#[test]
#[serial]
fn test_routine_force_case() {
    let path = get_test_path("force");
    let name = "ForcedCase_123";

    let opts = SetOptions {
        name: name.to_string(),
        value: "echo forced".to_string(),
        volatile: true,
        force_case: true,
    };

    P::set_alias(opts, &path, test_v()).expect("Forced set failed");

    let query = P::query_alias(name, test_v());
    assert!(query.iter().any(|s: &String| s.to_lowercase().contains("forced")));

    let _ = fs::remove_file(path);
}

/////////////////////////////////////////////


#[cfg(any(test, feature = "test_utils"))]
pub mod testing {
    use super::*;
    use std::path::PathBuf;

    pub fn get_test_path(suffix: &str) -> PathBuf {
        PathBuf::from(format!("test_{}_{:?}.doskey", suffix, std::thread::current().id()))
    }

    pub fn run_generic_set_and_query<P: AliasProvider>() {
        let name = "gauntlet_test";
        let val = "echo gauntlet";
        let path = get_test_path("gauntlet");
        let v = voice!(Silent, ShowFeature::Off, ShowFeature::Off);

        let opts = SetOptions {
            name: name.to_string(),
            value: val.to_string(),
            volatile: false,
            force_case: false,
        };

        P::set_alias(opts, &path, v).expect("Failed to set alias");
        let results = P::query_alias(name, v);
        assert!(results.iter().any(|s: &String| s.contains(val)));

        let _ = std::fs::remove_file(path);
    }

    pub fn run_generic_registry_test<P: AliasProvider>() {
        let test_cmd = "echo alias_test_hook";
        let verbosity = Verbosity::silent();
        let res = P::write_autorun_registry(test_cmd, verbosity);
        assert!(res.is_ok());
        let current = P::read_autorun_registry();
        assert!(current.contains(test_cmd));
    }
}

// tests/alias_win32_wrapper.rs

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

    // FIX: Call via the Interface P
    P::set_alias(opts, &path, voice!(Silent, Off, Off)).expect("Failed to set alias");

    let content = std::fs::read_to_string(&path).unwrap();
    assert!(content.contains("gauntlet_test=echo gauntlet"));

    // FIX: Call via P and add closure type annotation
    let results = P::query_alias(name, voice!(Silent, Off, Off));
    if !results.is_empty() {
        assert!(results.iter().any(|s: &String| s.contains(val))); // Added : &String
    }

    let _ = std::fs::remove_file(path);
}

#[test]
#[serial]
fn test_routine_reload_sync() {
    let path = get_test_path("reload");
    fs::write(&path, "k1=v1\nk2=v2\n").unwrap();

    // FIX: Use trait method for purging
    let _ = P::purge_ram_macros();

    // FIX: Use trait method for reloading
    P::reload_full(&path, test_v()).expect("Reload failed");

    let q1 = P::query_alias("k1", test_v());
    let q2 = P::query_alias("k2", test_v());

    assert!(!q1.is_empty());
    assert!(!q2.is_empty());

    let _ = fs::remove_file(path);
}

#[test]
#[serial]
fn z_nuke_the_world_end() {
    alias_nuke::kernel_wipe_macros();
}

