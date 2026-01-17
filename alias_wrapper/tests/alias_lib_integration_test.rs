// alias_wrapper/tests/alias_lib_integration_test.rs

// shared code start
extern crate alias_lib;

#[path = "../../tests/shared_test_utils.rs"]
mod test_suite_shared;
#[allow(unused_imports)]
use test_suite_shared::{MockProvider, MOCK_RAM, LAST_CALL, global_test_setup};
#[allow(unused_imports)]
use test_suite_shared::*;

#[path = "../../tests/state_restoration.rs"]
mod stateful;
#[cfg(test)]
#[ctor::ctor]
fn alias_lib_integration_test_init() {
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
fn alias_lib_integration_test_send() {
    eprintln!("[POST-FLIGHT] Warning: System state is finished.");
    stateful::post_flight_dec();
}


// 1. Pull in the public API of the library
#[allow(unused_imports)]
use alias_lib::*;
// use alias_wrapper::*;

/// ---------------------------------------------------------
/// 1. LOCAL TESTS (Specific to this crate)
/// ---------------------------------------------------------
#[test]
fn test_local_logic() {
    // No-op: Placeholder for future local tests
}

/// ---------------------------------------------------------
/// SHARED TESTS (The "Pragma" style include)
/// ---------------------------------------------------------
/// ---------------------------------------------------------
/// 2. THE BRAIN (Logic & Parsing)
/// ---------------------------------------------------------
#[cfg(test)]
#[path = "../../tests/alias_lib_tests.rs"]
mod shared_under_wrapper;
