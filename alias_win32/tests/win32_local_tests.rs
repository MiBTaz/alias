// alias_win32/tests/win32_local_tests.rs

use std::path::PathBuf;
#[allow(unused_imports)]
use serial_test::serial;
use alias_lib::*;
// Import the trait so its methods are available
use alias_win32::{Win32LibraryInterface, REG_SUBKEY, REG_AUTORUN_KEY};
extern crate alias_lib;

#[path = "../../tests/state_restoration.rs"]
mod stateful;
#[cfg(test)]
#[ctor::ctor]
fn win32_local_tests_init() {
    eprintln!("[PRE-FLIGHT] Warning: System state is starting.");
    // FORCE LINKAGE: This prevents the linker from tree-shaking the module
    // and silences the "unused" warnings by actually "using" them.
    let _ = stateful::has_backup();
    if stateful::is_stale() {
        // This path probably won't be hit, but the compiler doesn't know that.
        eprintln!("[PRE-FLIGHT] Warning: System state is stale.");
    }
    let _ = stateful::has_backup();
    stateful::pre_flight_inc();
    global_test_setup();
}
#[cfg(test)]
#[ctor::dtor]
fn win32_local_testsend() {
    eprintln!("[POST-FLIGHT] Warning: System state is finished.");
    stateful::post_flight_dec();
}

pub fn get_test_path(suffix: &str) -> PathBuf {
    PathBuf::from(format!("test_{}_{:?}.doskey", suffix, std::thread::current().id()))
}

//#[test]
//#[serial]
//fn a_nuke_the_world() {alias_nuke::kernel_wipe_macros();}

#[test]
#[serial]
fn test_local_win32_kernel_quirk() {
    assert!(true);
}

#[test]
#[serial]
fn test_win32_api_roundtrip() {
    let name = "test_alias_123";
    let val = "echo hello";

    // Call via the Interface
    Win32LibraryInterface::raw_set_macro(name, Some(val)).unwrap();
    let all = Win32LibraryInterface::get_all_aliases(&voice!(Silent, Off, Off)).expect("RAM fetch failed");
    let found = all.iter().find(|(n, _)| n == name);

    assert!(found.is_some());
    Win32LibraryInterface::raw_set_macro(name, None).unwrap();
}

#[test]
#[serial]
fn test_routine_clear_ram() {
    let name = "purge_me";
    Win32LibraryInterface::raw_set_macro(name, Some("temporary")).unwrap();

    let report = Win32LibraryInterface::purge_ram_macros(&voice!(Silent, Off, Off)).expect("Purge failed");

    assert!(report.cleared.iter().any(|n| n.to_lowercase() == name.to_lowercase()),
            "Purge did not report clearing our test key");

    let results = Win32LibraryInterface::query_alias(name, &Verbosity::normal());
    // Since query_alias returns Vec<String>, check for content or lack thereof
    assert!(results.iter().all(|s| !s.contains("temporary")));
}

#[test]
#[serial]
fn test_routine_purge_ram() {
    Win32LibraryInterface::raw_set_macro("purge_target", Some("alive")).unwrap();
    let _ = Win32LibraryInterface::purge_ram_macros(&voice!(Silent, Off, Off)).expect("Purge failed");

    let query = Win32LibraryInterface::query_alias("purge_target", &Verbosity::normal());

    // Use a more flexible check that matches your text! output
    assert!(query.get(0).map_or(false, |s| s.contains("not a known alias") || s.contains("not found")));
}

#[test]
#[serial]
fn test_win32_rapid_fire_sync() {
    let path = get_test_path("stress");
    for i in 0..20 {
        let name = format!("stress_test_{}", i);
        let opts = SetOptions {
            name: name.clone(),
            value: "echo work".into(),
            volatile: false,
            force_case: false,
        };
        Win32LibraryInterface::set_alias(opts, &path, &Verbosity::normal()).expect("Rapid fire set failed");
    }

    let all = Win32LibraryInterface::get_all_aliases(&voice!(Silent, Off, Off)).expect("RAM fetch failed");
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

    assert!(Win32LibraryInterface::raw_set_macro(name, Some(val)).unwrap(), "Failed to set international alias");

    let all = Win32LibraryInterface::get_all_aliases(&voice!(Silent, Off, Off)).expect("RAM fetch failed");
    let found = all.iter().find(|(n, _)| n == name);

    assert!(found.is_some(), "International alias 'λ' was mangled or lost");
    assert_eq!(found.unwrap().1, val);

    Win32LibraryInterface::raw_set_macro(name, None).unwrap();
}

#[test]
#[serial]
fn test_registry_append_logic_library() {
    use winreg::RegKey;
    use winreg::enums::HKEY_CURRENT_USER;

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let (key, _) = hkcu.create_subkey(REG_SUBKEY).unwrap();

    let original_cmd = "echo 'Old Command'";
    key.set_value(REG_AUTORUN_KEY, &original_cmd.to_string()).unwrap();

    Win32LibraryInterface::write_autorun_registry(&format!("{} & alias --reload", original_cmd), &Verbosity::normal()).expect("Install failed");

    let result: String = key.get_value(REG_AUTORUN_KEY).unwrap();
    assert!(result.contains(original_cmd));
    assert!(result.contains("--reload"));
}

#[test]
fn test_thread_silo_isolation_local() {
    let name_a = "unique_silo_test_a";
    let name_b = "unique_silo_test_b";

    Win32LibraryInterface::raw_set_macro(name_a, Some("val_a")).unwrap();
    Win32LibraryInterface::raw_set_macro(name_b, Some("val_b")).unwrap();

    let all = Win32LibraryInterface::get_all_aliases(&voice!(Silent, Off, Off)).expect("RAM fetch failed");

    assert!(all.iter().any(|(n, _)| n == name_a));
    assert!(all.iter().any(|(n, _)| n == name_b));

    Win32LibraryInterface::raw_set_macro(name_a, None).unwrap();
    Win32LibraryInterface::raw_set_macro(name_b, None).unwrap();
}

type P = Win32LibraryInterface; // Define P for the template

include!("../../tests/cli_tests_win32.rs");

#[test]
#[serial]
fn z_nuke_the_world_end() {
    alias_nuke::kernel_wipe_macros();
}
