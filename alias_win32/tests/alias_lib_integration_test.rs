// alias_win32/tests/tests.rs

// 1. Pull in the public API of the library
#[allow(unused_imports)]
use alias_lib::*;
// use alias_win32::*;

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
mod shared_under_win32;

