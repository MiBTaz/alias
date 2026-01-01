// alias_lib/tests/tests.rs
// 1. Pull in the public API of the library
#[allow(unused_imports)]
use alias_lib::*;

/// ---------------------------------------------------------
/// LOCAL TESTS (Specific to this crate)
/// ---------------------------------------------------------
#[test]
fn test_local_logic() {
    // No-op: Placeholder for future local tests
}

/// ---------------------------------------------------------
/// SHARED TESTS (The "Pragma" style include)
/// ---------------------------------------------------------
#[cfg(test)]
#[path = "../../tests/alias_lib_tests.rs"]
mod shared;
