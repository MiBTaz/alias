// alias_hybrid/tests/z_last_hybrid.rs

use std::path::Path;
use std::process::Command;
use assert_cmd::cargo_bin;
use assert_cmd::prelude::*;
use serial_test::serial;
use alias::HybridLibraryInterface; // Ensure this matches your crate name
use alias_lib::*;
#[allow(unused_imports)]
use alias_lib::ShowFeature;

#[cfg(test)]
#[ctor::ctor]
fn init() {
    unsafe {
        std::env::remove_var("ALIAS_FILE");
        std::env::remove_var("ALIAS_OPTS");
        std::env::remove_var("ALIAS_PATH");
    }
}

#[test]
#[serial(console)]
fn test_hybrid_volatile_bypass() {
    let alias_file = "hybrid_volatile.doskey";
    let mut cmd = Command::new(cargo_bin!("alias"));
    cmd.args(["--temp", "volatile_test", "success", "--file", alias_file]);
    cmd.assert().success();

    let content = std::fs::read_to_string(alias_file).unwrap_or_default();
    assert!(!content.contains("volatile_test"), "Volatile alias leaked to disk!");
    if Path::new(alias_file).exists() {
        std::fs::remove_file(alias_file).ok();
    }
}

#[test]
#[serial(console)]
fn test_hybrid_fallback_logic_robust() {
    let name = "fallback_check";
    let val = "found_it";
    let dummy_path = Path::new("temp_fallback.doskey");

    let opts = SetOptions {
        name: name.to_string(),
        value: val.to_string(),
        volatile: true,
        force_case: false,
    };

    // FIX: Replaced 'true' (bool) with 'voice!(Silent, Off, Off)'
    HybridLibraryInterface::set_alias(opts, dummy_path, &voice!(Silent, Off, Off))
        .expect("Internal set failed");

    // FIX: Replaced 'OutputMode::Silent' with 'voice!(Silent, Off, Off)'
    let results = HybridLibraryInterface::query_alias(name, &voice!(Silent, Off, Off));

    assert!(results.iter().any(|s| s.contains(val)), "Hybrid fallback logic failed internally");
}

#[test]
#[serial(console)]
fn test_ui_audit_logic() {
    let name = "audit_test";
    let val = "visible";
    let dummy_path = Path::new("audit.doskey");

    let opts = SetOptions {
        name: name.to_string(),
        value: val.to_string(),
        volatile: false,
        force_case: false,
    };

    // FIX: Replaced 'true' with 'voice!(Silent, Off, Off)'
    HybridLibraryInterface::set_alias(opts, dummy_path, &voice!(Silent, Off, Off))
        .expect("Internal set failed");

    // FIX: Added required Verbosity argument
    HybridLibraryInterface::alias_show_all(&voice!(Normal, Off, Off))
        .expect("UI Audit logic failed");

    if dummy_path.exists() {
        std::fs::remove_file(dummy_path).ok();
    }
}

type P = HybridLibraryInterface; // Define P for the template

include!("../../tests/cli_tests_win32.rs");
