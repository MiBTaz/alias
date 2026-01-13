// alias_wrapper/tests/alias_lib_integration_test.rs
#[cfg(test)]
#[ctor::ctor]
fn init() {
    unsafe {
        std::env::remove_var("ALIAS_FILE");
        std::env::remove_var("ALIAS_OPTS");
        std::env::remove_var("ALIAS_PATH");
    }
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
