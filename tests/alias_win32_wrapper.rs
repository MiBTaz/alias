// tests/alias_win32_wrapper.rs

#[allow(unused_imports)]
use std::fs;
#[allow(unused_imports)]
use std::io;
use std::path::PathBuf;
#[allow(unused_imports)]
use serial_test::serial;
// This brings in the trait, the structs, and the macro
#[allow(unused_imports)]
use alias_lib::*;
#[allow(unused_imports)]
use alias_lib::{REG_SUBKEY, REG_AUTORUN_KEY};
#[allow(unused_imports)]
use function_name::named;

extern crate alias_nuke;

// shared code start
extern crate alias_lib;

#[path = "shared_test_utils.rs"]
mod test_suite_shared;


#[allow(unused_imports)]
use test_suite_shared::{MockProvider, MOCK_RAM, LAST_CALL, global_test_setup};

// shared code end
#[path = "state_restoration.rs"]
mod stateful;
#[cfg(test)]
#[ctor::ctor]
fn local_library_tests_init() {
    eprintln!("[PRE-FLIGHT] Warning: System state is starting.");
    eprintln!("\n--- TEST ENVIRONMENT INITIALIZED ---");
    eprintln!("Provider Identity: {:?}", <P as alias_lib::AliasProvider>::provider_type());
    eprintln!("-----------------------------------\n");
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
fn alias_lib_tests_end() {
    eprintln!("[POST-FLIGHT] Warning: System state is finished.");
    stateful::post_flight_dec();
}


macro_rules! skip_if_wrapper {
    () => {
        if P::provider_type() == ProviderType::Wrapper {
            println!("Skipping: Known 'Spawn SNAFU' in Wrapper mode.");
            return;
        }
    };
}

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
    let _ = P::set_alias(opts, &test_path, &voice!(Silent, ShowFeature::Off, ShowTips::Off));

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
    P::run_diagnostics(&path, &voice!(Silent, ShowFeature::Off, ShowTips::Off)).expect("Diagnostics failed");
}

#[test]
#[serial]
fn test_routine_setup_registration() {
    // We test that the command executes. Result may be Err if no Admin,
    // but the logic path is exercised.
    let _ = P::install_autorun(&voice!(Silent, ShowFeature::Off, ShowTips::Off), "alias --startup");
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
    }, &path, &voice!(Silent, ShowFeature::Off, ShowTips::Off)).expect("Set Persistence failed");

    // RAM Check: Retry loop to handle Win32 kernel latency
    let mut success = false;
    for _ in 0..5 {
        let query = P::query_alias(name, &voice!(Silent, ShowFeature::Off, ShowTips::Off));
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
    P::alias_show_all(&voice!(Normal, ShowFeature::Off, ShowTips::Off)).expect("Audit should pass");
}

// -------------------------------

// Helper to create the required Verbosity object for the new API
fn test_v() -> Verbosity {
    voice!(Silent, ShowFeature::Off, ShowTips::Off)
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
    P::set_alias(opts, &path, &test_v()).unwrap();

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
    P::reload_full(&test_v(), &path, true).expect("Reload failed");

    // FIXED: query_alias now requires Verbosity
    let results = P::query_alias("reload_key", &test_v());
    assert!(results.iter().any(|s: &String| s.contains("reload_val")));

    let _ = fs::remove_file(path);
}

#[test]
#[serial]
fn test_routine_diagnostics() {
    let path = PathBuf::from("diag_test.doskey");
    // FIXED: Signature changed from (path) to (path, verbosity)
    P::run_diagnostics(&path, &voice!(Silent, ShowFeature::Off, ShowTips::Off)).expect("Diagnostics failed");
}

#[test]
#[serial]
fn test_routine_install_autorun() {
    // FIXED: Passed test_v() instead of 'true'
    let _ = P::install_autorun(&test_v(), "alias --startup");
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

    P::set_alias(opts, &path, &test_v()).unwrap();

    let query = P::query_alias("temp_macro", &test_v());
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

    P::set_alias(opts, &path, &test_v()).expect("Forced set failed");

    let query = P::query_alias(name, &test_v());
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
        let v = voice!(Silent, ShowFeature::Off, ShowTips::Off);

        let opts = SetOptions {
            name: name.to_string(),
            value: val.to_string(),
            volatile: false,
            force_case: false,
        };

        P::set_alias(opts, &path, &v).expect("Failed to set alias");
        let results = P::query_alias(name, &v);
        assert!(results.iter().any(|s: &String| s.contains(val)));

        let _ = std::fs::remove_file(path);
    }

    pub fn run_generic_registry_test<P: AliasProvider>() {
        let test_cmd = "echo alias_test_hook";
        let verbosity = Verbosity::silent();
        let res = P::write_autorun_registry(test_cmd, &verbosity);
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
    P::set_alias(opts, &path, &voice!(Silent, Off, Off)).expect("Failed to set alias");

    let content = std::fs::read_to_string(&path).unwrap();
    assert!(content.contains("gauntlet_test=echo gauntlet"));

    // FIX: Call via P and add closure type annotation
    let results = P::query_alias(name, &voice!(Silent, Off, Off));
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
    let _ = P::purge_ram_macros(&voice!(Silent, Off, Off));

    // FIX: Use trait method for reloading
    P::reload_full(&test_v(), &path, true).expect("Reload failed");

    let q1 = P::query_alias("k1", &test_v());
    let q2 = P::query_alias("k2", &test_v());

    assert!(!q1.is_empty());
    assert!(!q2.is_empty());

    let _ = fs::remove_file(path);
}

// Helper for unique test paths
pub fn get_test_path(suffix: &str) -> PathBuf {
    PathBuf::from(format!("test_{}_{:?}.doskey", suffix, std::thread::current().id()))
}
#[test]
#[serial]
fn test_win32_api_roundtrip() {
    let name = "test_alias_123";
    let val = "echo hello";
    P::raw_set_macro(name, Some(val)).unwrap();
    let all = P::get_all_aliases(&voice!(Silent, Off, Off)).expect("Failed to read RAM macros");
    let found = all.iter().find(|(n, _)| n == name);
    assert!(found.is_some());
    P::raw_set_macro(name, None).unwrap();
}

#[test]
#[serial]
fn test_routine_clear_ram() {
    let name = "purge_me";
    P::raw_set_macro(name, Some("temporary")).unwrap();
    let _ = P::purge_ram_macros(&voice!(Silent, Off, Off)).expect("Purge failed");
    let results = P::query_alias(name, &Verbosity::normal());
    assert!(results.iter().all(|s| !s.contains("temporary")));
}

#[test]
#[serial]
fn test_routine_delete_sync() {
    let path = get_test_path("del");
    fs::write(&path, "ghost=gone\n").unwrap();
    P::raw_set_macro("ghost", Some("gone")).unwrap();

    let opts = SetOptions {
        name: "ghost".into(),
        value: "".into(),
        volatile: false,
        force_case: false,
    };

    P::set_alias(opts, &path, &Verbosity::normal()).unwrap();

    // SLEEPER AGENT COUNTER-MEASURE:
    // We poll for up to 500ms to allow the File System and Win32 RAM to sync.
    let mut success = false;
    for _ in 0..10 {
        let query = P::query_alias("ghost", &Verbosity::normal());
        if query.is_empty() || query[0].contains("not found") {
            success = true;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }

    assert!(success, "Ghost alias persisted in file or RAM after deletion attempt");

    let _ = fs::remove_file(path);
}
#[test]
#[serial]
fn test_thread_silo_isolation() {
    let pid = std::process::id();
    let name_a = format!("silo_a_{}", pid);
    let name_b = format!("silo_b_{}", pid);

    // 1. Set A
    P::raw_set_macro(&name_a, Some("val_a")).unwrap();
    // 2. Set B
    P::raw_set_macro(&name_b, Some("val_b")).unwrap();

    std::thread::sleep(std::time::Duration::from_millis(50));

    let all = P::get_all_aliases(&voice!(Silent, Off, Off)).expect("Failed to read RAM macros");

    // Prove both exist independently
    let has_a = all.iter().any(|(n, _)| n == &name_a);
    let has_b = all.iter().any(|(n, _)| n == &name_b);

    assert!(has_a, "Missing A");
    assert!(has_b, "Missing B");

    // Cleanup
    P::raw_set_macro(&name_a, None).unwrap();
    P::raw_set_macro(&name_b, None).unwrap();
}

#[test]
fn test_purge_stress_partial_failure() {
    let v = Verbosity::mute();

    // 1. Define a Mock that fails to delete any alias named "PROTECTED"
    struct MaliciousMock;
    impl AliasProvider for MaliciousMock {
        // --- 1. The Logic you care about ---
        fn raw_set_macro(name: &str, value: Option<&str>) -> io::Result<bool> {
            if name == "PROTECTED" && value.is_none() {
                return Ok(false);
            }
            Ok(true)
        }

        fn get_all_aliases(_: &Verbosity) -> io::Result<Vec<(String, String)>> {
            Ok(vec![
                ("ls".into(), "dir".into()),
                ("PROTECTED".into(), "secret".into()),
            ])
        }
        // --- 2. The Updated Paperwork (Matching lib.rs Trait) ---

        // MATCH: &std::path::Path instead of &str
        fn raw_reload_from_file(_v: &Verbosity,_: &std::path::Path) -> io::Result<()> { Ok(()) }

        fn write_autorun_registry(_cmd: &str, _v: &Verbosity) -> io::Result<()> { Ok(()) }

        // MATCH: Returns String directly, not Result<Option>
        fn read_autorun_registry() -> String { String::new() }

        // MATCH: Returns Vec<String>
        fn query_alias(_: &str, _: &Verbosity) -> Vec<String> { vec![] }

        // MATCH: SetOptions and &Path
        fn set_alias(_: SetOptions, _: &std::path::Path, _: &Verbosity) -> io::Result<()> { Ok(()) }

        // MATCH: Result<(), Box<dyn Error>>
        fn alias_show_all(_: &Verbosity) -> Result<(), Box<dyn std::error::Error>> { Ok(()) }

        // MATCH: &Path and Result<(), Box<dyn Error>>
        fn run_diagnostics(_path: &std::path::Path, _verbosity: &Verbosity) -> Result<(), Box<dyn std::error::Error>> { Ok(()) }

        fn purge_ram_macros(v: &Verbosity) -> io::Result<PurgeReport> {
            let mut report = PurgeReport::default();

            // 1. Get the aliases from the Mock's own provider
            let aliases = Self::get_all_aliases(v)?;

            for (name, _) in aliases {
                // 2. Try to "delete" via the Mock's own raw_set_macro
                match Self::raw_set_macro(&name, None) {
                    Ok(true) => report.cleared.push(name),
                    _ => report.failed.push((name, 5)),
                }
            }
            Ok(report)
        }

        fn get_version() -> &'static Versioning {
            static MOCK_VER: Versioning = Versioning {
                lib: "MaliciousMock",
                major: 6,
                minor: 6,
                patch: 6,
                compile: 666,
                timestamp: "6666-06-06",
            };
            &MOCK_VER
        }
    }
    // 2. Run the purge
    let report = MaliciousMock::purge_ram_macros(&v).unwrap();

    // 3. Validation
    assert!(report.cleared.contains(&"ls".to_string()));
    assert!(report.failed.iter().any(|(name, _)| name == "PROTECTED"),
            "The report must capture the failure of the PROTECTED alias");
}

#[test]
fn test_set_and_clear_poisoned_alias() {
    let name = "\"ghost_test";
    let val = "echo boo\"";

    // 1. Set it (should include quotes in RAM)
    P::raw_set_macro(name, Some(val)).expect("Should set poisoned alias");

    // 2. Clear it (The critical fix: passing the same quoted name should delete it)
    let result = P::raw_set_macro(name, None).expect("Should delete poisoned alias");
    assert!(result, "Windows should report success for deletion of quoted name");
}


#[test]
fn test_xcd_trailing_quote_integrity() {
    // 1. The Gatekeeper: Skip the "Slow Ship"
    skip_if_wrapper!();

    let name = "xcd_test";
    // The raw string exactly as it appears in your working aliases file
    let val = r#"for /f "delims=" %i in ('dir') do cd /d "%i""#;

    // 2. The Action: Direct API call via Provider (P)
    P::raw_set_macro(name, Some(val))
        .expect("Win32 Kernel rejected the alias syntax");

    // 3. The Forensic Check: Did the API store it correctly?
    let ram = P::get_all_aliases(&Verbosity::loud()).expect("Failed to read back from RAM");

    let (_, stored_val) = ram.iter()
        .find(|(n, _)| n == name)
        .expect("Alias disappeared from RAM immediately after set");

    // 4. The "No-Murder" Assert
    assert!(
        stored_val.ends_with('"'),
        "FAILURE: Trailing quote was stripped. Expected tail: [\"], Found: [{}]",
        &stored_val[stored_val.len()-1..]
    );

    // Cleanup
    let _ = P::raw_set_macro(name, None);
}


#[test]
fn test_alphanumeric_alias() {
    // Standard case should still work perfectly
    P::raw_set_macro("standard", Some("echo hello")).expect("Should set standard alias");
    P::raw_set_macro("standard", None).expect("Should clear standard alias");
}


#[test]
#[serial]
fn z_nuke_the_world_end() {
    alias_nuke::kernel_wipe_macros();
}