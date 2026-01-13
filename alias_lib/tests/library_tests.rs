// alias_lib/tests/library_tests.rs

// 1. Pull in the public API of the library
#[cfg(test)]
#[ctor::ctor]
fn init() {
    unsafe {
        std::env::remove_var("ALIAS_FILE");
        std::env::remove_var("ALIAS_OPTS");
        std::env::remove_var("ALIAS_PATH");
    }
}

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
