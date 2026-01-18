// alias_wrapper/tests/wrapper_local_tests.rs

#[allow(unused_imports)]
use function_name::named;
use alias_lib::*;
use serial_test::serial;
#[allow(unused_imports)]
use alias_lib::ShowFeature::{self, On, Off};
use alias_wrapper::WrapperLibraryInterface as P;

// shared code start
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
fn win32_local_tests_end() {
    eprintln!("[POST-FLIGHT] Warning: System state is finished.");
    stateful::post_flight_dec();
}

#[path = "../../tests/shared_test_utils.rs"]
mod test_suite_shared;
#[allow(unused_imports)]
use test_suite_shared::{MOCK_RAM, MockProvider, LAST_CALL, global_test_setup};

// shared code end

// use ctor to wipe envvars
#[cfg(test)]
#[ctor::ctor]
fn init_wrapper_local() {
    global_test_setup();
}

#[test]
#[serial]
fn test_wrapper_strike_direct() {
    let path = get_test_path("local_strike");
    let opts = SetOptions {
        name: "local_test".into(),
        value: "echo wrapper_direct".into(),
        volatile: false,
        force_case: false,
    };

    // FIX: Call via the Interface 'P'
    P::set_alias(opts, &path, &voice!(Silent, ShowFeature::Off, ShowTips::Off)).expect("Wrapper strike failed");

    // FIX: Add explicit type &String to the closure
    let results = P::query_alias("local_test", &voice!(Silent, Off, Off));
    assert!(results.iter().any(|r: &String| r.contains("wrapper_direct")));

    let _ = std::fs::remove_file(path);
}

#[test]
#[serial]
fn test_wrapper_complex_chain() {
    let path = get_test_path("chain");
    let opts = SetOptions {
        name: "chain".into(),
        value: "echo part1 & echo part2".into(),
        volatile: true,
        force_case: false,
    };

    // FIX: Call via the Interface 'P'
    P::set_alias(opts, &path, &voice!(Silent, Off, Off)).expect("Wrapper failed to set complex alias");

    // FIX: Call via the Interface 'P' and add type hint
    let results = P::query_alias("chain", &voice!(Silent, Off, Off));
    assert!(results.iter().any(|r: &String| r.contains("echo part2")), "Command chain was truncated or mangled");
}

#[test]
#[serial]
fn test_wrapper_setup_flow() {
    // FIX: Call via the Interface 'P'
    P::install_autorun(&voice!(Silent, Off, Off), "alias --startup").expect("Wrapper install failed");
}

// Helper kept local
fn get_test_path(suffix: &str) -> std::path::PathBuf {
    std::path::PathBuf::from(format!("test_wrapper_{}_{:?}.doskey", suffix, std::thread::current().id()))
}


include!("../../tests/cli_tests_wrapper.rs");
