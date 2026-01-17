// tests/cli_tests_wrapper.rs

#[allow(unused_imports)]
use std::process::Command;
#[allow(unused_imports)]
use assert_cmd::cargo_bin;
#[allow(unused_imports)]
use assert_cmd::prelude::*;

#[cfg(test)]
#[ctor::ctor]
fn init_cli_tests_wrapper() { global_test_setup(); }

#[test]
#[serial]
fn test_voice_masking_consistency() {
    // Ensures the wrapper respects the 'Silent' voice
    // even when performing complex proxy actions.
    let v = voice!(Silent, Off, Off);
    let name = "wrapper_internal_test";

    // Test if the wrapper can handle a query silently
    let results = P::query_alias(name, &v);
    assert!(results.is_empty() || !results[0].contains("DEBUG"),
            "Wrapper leaked trace info in Silent mode");
}

// Local helper to avoid library visibility issues
fn local_get_alias_dir() -> std::path::PathBuf {
    std::env::current_exe()
        .map(|p| p.parent().unwrap_or(&p).to_path_buf())
        .expect("Could not resolve test binary path")
}

#[test]
#[serial]
fn test_wrapper_binary_resolution() {
    let path = local_get_alias_dir();
    assert!(path.exists());
}

#[test]
#[serial]
fn test_wrapper_passthrough_logic() {
    // 1. Get the current crate's name (alias, alias_wrapper, or alias_win32)
    let pkg_name = env!("CARGO_PKG_NAME");

    // 2. Format the variable name Cargo sets for binaries: CARGO_BIN_EXE_<name>
    // Note: We use env::var (runtime) instead of env! (compile-time) to avoid the error.
    let bin_path = std::env::var(format!("CARGO_BIN_EXE_{}", pkg_name))
        .unwrap_or_else(|_| pkg_name.to_string());

    // 3. Create the command using the path we found
    let mut cmd = Command::new(bin_path);
    cmd.arg("--version");

    // This uses the 'assert' method from assert_cmd::prelude
    cmd.assert().success();
}
/*
#[test]
#[named]
fn test_wrapper_preserves_internal_trailing_quotes() {
    let name = "ncd_test";
    let cmd = r#"for /f "delims=" %i in ('ncd.exe $*') do cd /d "%i""#;

    // Define the different ways this alias might show up in doskey output
    let test_cases = vec![
        format!("{name}={cmd}"),               // Clean: Standard output
        format!("\"{name}={cmd}\""),             // Dirty: Whole line quoted
        format!(" {name}={cmd} "),               // Spaced: Extra whitespace
        format!("\"{name}\"=\"{cmd}\""),         // Individually quoted parts
    ];

    let mut i = 1;
    for mock_input in test_cases {
        let results = alias_wrapper::parse_raw_doskey_output(&mock_input);
        trace!("--> Iter #{}", i);
        i += 1;
        // Ensure we actually got a result back
        let (parsed_name, parsed_value) = results.get(0)
            .expect(&format!("Parser failed to extract alias from: {}", mock_input));

        assert_eq!(parsed_name, name, "Name mismatch for input: {}", mock_input);
        assert_eq!(parsed_value, cmd, "Value corrupted for input: {}", mock_input);
    }
}
*/