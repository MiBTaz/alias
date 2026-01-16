// alias_wrapper/tests/alias_lib_integration_test.rs

// shared code start
extern crate alias_lib;

#[path = "../../tests/shared_test_utils.rs"]
mod test_suite_shared;
#[allow(unused_imports)]
use test_suite_shared::{MockProvider, MOCK_RAM, LAST_CALL, global_test_setup};
#[allow(unused_imports)]
use test_suite_shared::*;

// shared code end
#[cfg(test)]
#[ctor::ctor]
fn alias_lib_integration_test() {
    global_test_setup();
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
