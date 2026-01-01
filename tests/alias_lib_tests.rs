// src/tests/alias_tests.rs

use crate::*;
use std::env;
use std::fs;
use std::path::PathBuf;

fn mock_set(name: &str, value: &str) -> AliasAction {
    AliasAction::Set(SetOptions {
        name: name.to_string(),
        value: value.to_string(),
        volatile: false,
        force_case: false,
    })
}

// --- PARSER TESTS ---

#[test]
fn test_show_all() {
    let args = vec!["alias".to_string()];
    assert_eq!(parse_alias_args(&args), (AliasAction::ShowAll, false));
}

#[test]
fn test_query_alias() {
    let args = vec!["alias".to_string(), "ls".to_string()];
    assert_eq!(parse_alias_args(&args), (AliasAction::Query("ls".to_string()), false));
}

#[test]
fn test_set_with_equals() {
    let args = vec!["alias".to_string(), "gs=git".to_string(), "status".to_string()];
    assert_eq!(
        parse_alias_args(&args),
        (
            AliasAction::Set(SetOptions {
                name: "gs".to_string(),
                value: "git status".to_string(),
                volatile: false,
                force_case: false,
            }),
            false
        )
    );
}

#[test]
fn test_set_with_space() {
    let args = vec!["alias".to_string(), "vi".to_string(), "nvim".to_string(), "-o".to_string()];
    assert_eq!(
        parse_alias_args(&args),
        ( // <--- ERROR 1: Needed opening parenthesis for the tuple
          AliasAction::Set(SetOptions {
              name: "vi".to_string(),      // <--- ERROR 3: Needed .to_string()
              value: "nvim -o".to_string(), // <--- ERROR 3: Needed .to_string()
              volatile: false,
              force_case: false,           // <--- ERROR 2: Was missing / duplicated
          }),
          false
        ) // <--- ERROR 1: Needed closing parenthesis for the tuple
    );
}

#[test]
fn test_delete_syntax() {
    let args = vec!["alias".to_string(), "junk=".to_string()];
    assert_eq!(
        parse_alias_args(&args),
        (
            AliasAction::Set(SetOptions {
                name: "junk".to_string(),
                value: "".to_string(),
                volatile: false,
                force_case: false,
            }),
            false
        )
    );
}

#[test]
fn test_invalid_empty_name() {
    let args = vec!["alias".to_string(), "=something".to_string()];
    assert_eq!(parse_alias_args(&args), (AliasAction::Invalid, false));
}

#[test]
fn test_long_opts_only() {
    assert_eq!(parse_alias_args(&vec!["alias".into(), "--edalias".into()]), (AliasAction::Edit(None), false));
    assert_eq!(parse_alias_args(&vec!["alias".into(), "--help".into()]), (AliasAction::Help, false));
    assert_eq!(parse_alias_args(&vec!["alias".into(), "--reload".into()]), (AliasAction::Reload, false));
}

#[test]
fn test_help_and_invalid_flags() {
    assert_eq!(parse_alias_args(&vec!["alias".into(), "--help".into()]), (AliasAction::Help, false));
    assert_eq!(parse_alias_args(&vec!["alias".into(), "--unknown".into()]), (AliasAction::Invalid, false));
}

#[test]
fn test_short_flags_are_invalid_for_safety() {
    let args = vec!["alias".to_string(), "-e".to_string()];
    let (action, _) = parse_alias_args(&args);
    // We now expect Invalid to prevent accidental alias execution
    assert_eq!(action, AliasAction::Invalid);
}

#[test]
fn test_editor_synonyms() {
    assert_eq!(parse_alias_args(&vec!["alias".into(), "--edalias".into()]), (AliasAction::Edit(None), false));
    assert_eq!(parse_alias_args(&vec!["alias".into(), "--edaliases".into()]), (AliasAction::Edit(None), false));
}

#[test]
fn test_edit_with_override() {
    let action = parse_alias_args(&vec!["alias".into(), "--edalias=code".into()]);
    assert_eq!(action, (AliasAction::Edit(Some("code".into())), false));
}

#[test]
fn test_edit_synonym_with_override() {
    let action = parse_alias_args(&vec!["alias".into(), "--edaliases=nvim".into()]);
    assert_eq!(action, (AliasAction::Edit(Some("nvim".into())), false));
}

#[test]
fn test_quiet_flag_detection() {
    let args1 = vec!["alias".into(), "--quiet".into(), "gs=git status".into()];
    let (action1, quiet1) = parse_alias_args(&args1);

    assert!(quiet1);
    assert_eq!(
        action1,
        AliasAction::Set(SetOptions {
            name: "gs".into(),
            value: "git status".into(),
            volatile: false,
            force_case: false,
        })
    );
}

#[test]
fn test_set_complex_value_with_spaces() {
    let args = vec!["alias".into(), "my_cmd".into(), "echo".into(), "hi".into()];
    let (action, _) = parse_alias_args(&args);

    // Destructure the tuple variant 'Set' to get the 'opts' struct
    if let AliasAction::Set(opts) = action {
        assert_eq!(opts.name, "my_cmd");
        assert_eq!(opts.value, "echo hi");
    } else {
        panic!("Expected AliasAction::Set, got {:?}", action);
    }
}

// --- FILE SYSTEM TESTS ---

#[test]
fn test_path_not_empty_if_exists() {
    if let Some(path) = get_alias_path() {
        assert!(!path.to_string_lossy().is_empty());
    }
}

#[test]
fn test_path_priority_logic() {
    if let Ok(custom) = std::env::var("DOSKEY_ALIASES") {
        let path = get_alias_path().expect("Should find path");
        if std::path::Path::new(&custom).exists() {
            assert!(path.to_string_lossy().contains(&custom) || path == std::path::PathBuf::from(&custom));
        }
    }
}

#[test]
fn test_is_path_healthy_with_readonly() {
    let temp_file = env::temp_dir().join("readonly_test.doskey");
    fs::write(&temp_file, "test=val").unwrap();

    let mut perms = fs::metadata(&temp_file).unwrap().permissions();
    perms.set_readonly(true);
    fs::set_permissions(&temp_file, perms).unwrap();

    assert!(!is_path_healthy(&temp_file));

    // Re-fetch metadata to avoid "borrow of moved value"
    let mut cleanup_perms = fs::metadata(&temp_file).unwrap().permissions();
    cleanup_perms.set_readonly(false);
    fs::set_permissions(&temp_file, cleanup_perms).unwrap();
    fs::remove_file(temp_file).unwrap();
}

#[test]
fn test_reload_flag() {
    let args = vec!["alias".into(), "--reload".into()];
    assert_eq!(parse_alias_args(&args), (AliasAction::Reload, false));
}

#[test]
fn test_multiple_quiet_flags() {
    // Users might accidentally or jokingly use it twice
    let args = vec!["alias".into(), "--quiet".into(), "ls".into(), "--quiet".into()];
    let (action, quiet) = parse_alias_args(&args);
    assert!(quiet);
    assert_eq!(action, AliasAction::Query("ls".into()));
}

#[test]
fn test_set_multiple_equals() {
    // Testing: alias logic="a=b" (The first equals should be the split point)
    let args = vec!["alias".into(), "logic=a=b".into()];
    let (action, _) = parse_alias_args(&args);

    assert_eq!(
        action,
        AliasAction::Set(SetOptions {
            name: "logic".into(),
            value: "a=b".into(),
            volatile: false,
            force_case: false,
        })
    );
}

#[test]
fn test_mixed_case_quiet() {
    // Ensure --QUIET or --qUiEt still triggers
    let args = vec!["alias".into(), "--QuIeT".into(), "--reload".into()];
    let (action, quiet) = parse_alias_args(&args);
    assert!(quiet);
    assert_eq!(action, AliasAction::Reload);
}

#[test]
fn test_only_quiet_no_command() {
    // alias --quiet (should behave like 'alias' but quiet)
    let args = vec!["alias".into(), "--quiet".into()];
    let (action, quiet) = parse_alias_args(&args);
    assert!(quiet);
    assert_eq!(action, AliasAction::ShowAll);
}


#[test]
fn test_opts_injection_logic() {
    // Simulating: ALIAS_OPTS="--quiet" and user types "alias g=git status"
    let mut args = vec!["alias".to_string(), "g=git".to_string(), "status".to_string()];
    let extra_opts = vec!["--quiet".to_string()];

    // Injects "--quiet" at index 1
    args.splice(1..1, extra_opts);

    let (action, quiet) = parse_alias_args(&args);
    assert!(quiet);

    // FIX: Change { name, value } to (opts)
    if let AliasAction::Set(opts) = action {
        assert_eq!(opts.name, "g");
        assert_eq!(opts.value, "git status");
        // Extra credit: verify flags were defaulted correctly
        assert!(!opts.volatile);
        assert!(!opts.force_case);
    } else {
        panic!("Failed to parse Set action after injection. Got: {:?}", action);
    }
}

#[test]
fn test_empty_value_is_delete() {
    let args = vec!["alias".to_string(), "old_alias=".to_string()];
    let (action, _) = parse_alias_args(&args);

    // Pattern match on the new Tuple Variant
    if let AliasAction::Set(opts) = action {
        assert_eq!(opts.name, "old_alias");
        assert_eq!(opts.value, ""); // Verified: empty value is captured correctly
    } else {
        panic!("Expected AliasAction::Set, got {:?}", action);
    }
}

#[test]
fn test_space_separator_set() {
    // Testing the "alias name value" alternate syntax
    let args = vec!["alias".to_string(), "ll".to_string(), "ls".to_string(), "-la".to_string()];
    let (action, _) = parse_alias_args(&args);

    // FIX: Match on the tuple variant 'Set' and access the 'opts' struct
    if let AliasAction::Set(opts) = action {
        assert_eq!(opts.name, "ll");
        assert_eq!(opts.value, "ls -la");
    } else {
        panic!("Space separator parsing failed. Got: {:?}", action);
    }
}

#[test]
fn test_path_with_spaces_healthy() {
    // Ensure our path checker doesn't choke on spaces (common in Windows)
    let path = PathBuf::from(r"C:\Program Files\Common Files\test.doskey");
    // This is a unit test, so we just check the logic doesn't crash
    let _ = is_path_healthy(&path);
}

#[test]
fn test_quiet_injection_from_env() {
    // 1. Simulate the initial args from the OS (alias g status)
    let mut args = vec![
        "alias".to_string(),
        "g".to_string(),
        "status".to_string()
    ];

    // 2. Simulate the ALIAS_OPTS="--quiet" injection logic from main()
    let env_opts = "--quiet";
    let extra: Vec<String> = env_opts.split_whitespace().map(String::from).collect();
    if !extra.is_empty() {
        args.splice(1..1, extra);
    }

    // 3. Parse the modified args
    let (action, quiet) = parse_alias_args(&args);

    // 4. Assertions
    assert!(quiet, "The quiet flag should be true when --quiet is injected");

    // FIX: Match on the tuple variant (opts)
    if let AliasAction::Set(opts) = action {
        assert_eq!(opts.name, "g");
        assert_eq!(opts.value, "status");
    } else {
        panic!("Action should have been AliasAction::Set, but got {:?}", action);
    }
}

#[test]
fn test_quiet_extraction() {
    let args = vec!["alias".to_string(), "--quiet".to_string(), "g=status".to_string()];
    let (action, quiet) = parse_alias_args(&args);

    assert!(quiet); // The bool is successfully caught

    // FIX: Match against the new Tuple Variant and Struct
    assert_eq!(
        action,
        AliasAction::Set(SetOptions {
            name: "g".into(),
            value: "status".into(),
            volatile: false,
            force_case: false,
        })
    );
}

#[test]
fn test_edit_empty_override() {
    // alias --edalias=
    let action = parse_alias_args(&vec!["alias".into(), "--edalias=".into()]);
    // It should probably treat an empty override as None (default editor)
    if let (AliasAction::Edit(Some(editor)), _) = action {
        assert!(!editor.is_empty(), "Editor name should not be empty");
    }
}

#[test]
fn test_get_alias_path_logic() {
    if let Some(path) = get_alias_path() {
        let path_str = path.to_string_lossy().to_lowercase();
        // If the user has a custom ENV VAR, allow it; otherwise, expect .doskey
        if std::env::var(ENV_ALIAS_FILE).is_err() {
            assert!(path_str.ends_with(".doskey"), "Default path should end with .doskey");
        } else {
            assert!(!path_str.is_empty(), "Custom path from ENV should not be empty");
        }
    }
}

#[test]
fn test_path_extension_validity() {
    if let Some(path) = get_alias_path() {
        // Only enforce the extension check if the user hasn't overridden the file via ENV
        if std::env::var(ENV_ALIAS_FILE).is_err() {
            let extension = path.extension().and_then(|s| s.to_str()).unwrap_or("");
            assert_eq!(extension, "doskey");
        }
    }
}

#[test]
fn test_alias_deletion_parsing() {
    let input = "cdx=";
    let parts: Vec<&str> = input.splitn(2, '=').collect();

    assert_eq!(parts[0], "cdx");
    // If this is an empty string, your code MUST call the delete logic
    assert_eq!(parts[1], "");
}

#[test]
fn test_deletion_logic_path() {
    let name = "cdx";
    let value = "";
    let input = format!("{}={}", name, value);

    let args = vec!["alias".to_string(), input];

    // parse_alias_args returns (AliasAction, bool)
    let action = parse_alias_args(&args).0;

    // FIX: Destructure the tuple variant 'Set' to get 'opts'
    if let AliasAction::Set(opts) = action {
        assert_eq!(opts.name, name);
        assert_eq!(opts.value, value);
    } else {
        panic!("Parser failed to identify '{}' as a Set/Delete action", args[1]);
    }
}

#[test]
fn test_logic_file_deletion() {
    let original = "ls=dir\ncdx=old_cmd\ngs=git status\n";
    let name = "cdx";
    let value = ""; // The delete signal

    let result = calculate_new_file_state(original, name, value);

    assert!(!result.contains("cdx="), "The result should not contain the deleted alias");
    assert!(result.contains("ls=dir"), "Other aliases should remain untouched");
    assert!(result.contains("gs=git status"), "Other aliases should remain untouched");
}

#[test]
fn test_logic_file_update() {
    let original = "ls=dir\n";
    let name = "ls";
    let value = "ls --color=auto";

    let result = calculate_new_file_state(original, name, value);

    assert!(result.contains("ls=ls --color=auto"), "The alias should be updated");
    assert!(!result.contains("ls=dir\n"), "The old version should be gone");
}

#[test]
fn test_split_alias_logic() {
    // Ensure we only split on the FIRST equals sign
    let input = "cdx=set \"VAR=VAL\" & chdir";
    let (name, value) = input.split_once('=').unwrap();
    assert_eq!(name, "cdx");
    assert_eq!(value, "set \"VAR=VAL\" & chdir");
}

#[test]
fn test_clear_string_formatting() {
    let name = "  rust  ";
    let clean_name = name.trim();
    let doskey_cmd = format!("{}=", clean_name);
    // This is the string that kills a macro in RAM
    assert_eq!(doskey_cmd, "rust=");
}

#[test]
fn test_reload_line_parsing() {
    // Simulate doskey /macros output
    let raw_output = "ls=dir /w\ncd=cd /d $*\nghost=";
    let names: Vec<&str> = raw_output
        .lines()
        .filter_map(|l| l.split_once('='))
        .map(|(name, _)| name.trim())
        .filter(|n| !n.is_empty())
        .collect();

    assert_eq!(names, vec!["ls", "cd", "ghost"]);
}

#[test]
fn test_parse_set_equals() {
    let args = vec!["alias".to_string(), "rust=cargo".to_string()];
    let (action, _) = parse_alias_args(&args);

    match action {
        // FIX: Match the tuple variant and access the inner struct
        AliasAction::Set(opts) => {
            assert_eq!(opts.name, "rust");
            assert_eq!(opts.value, "cargo");
        }
        _ => panic!("Expected Set action, but got: {:?}", action),
    }
}

#[test]
fn test_parse_set_space() {
    let args = vec!["alias".to_string(), "rust".to_string(), "cargo".to_string()];
    let (action, _) = parse_alias_args(&args);

    match action {
        // FIX: Match the tuple variant and access the opts fields
        AliasAction::Set(opts) => {
            assert_eq!(opts.name, "rust");
            assert_eq!(opts.value, "cargo");
        }
        _ => panic!("Expected Set action, got {:?}", action),
    }
}

#[test]
fn test_quiet_flag_positioning() {
    let args = vec!["alias".to_string(), "--quiet".to_string(), "ls=dir".to_string()];
    let (_, quiet) = parse_alias_args(&args);
    assert!(quiet);
}

#[test]
fn test_complex_deletion_logic() {
    // This tests the "Ghost" logic without needing set_alias or a real disk
    let name = "cdx";
    let value = ""; // Delete signal
    let complex_alias = "cdx=FOR /F tokens=* %i IN ('v:\\lbin\\ncd.exe $*') DO @(set OLDPWD=%CD% & chdir /d %i)\n";
    let other_alias = "ls=dir\n";
    let original_content = format!("{}{}", complex_alias, other_alias);

    // Call the PURE logic from the lib
    let result = calculate_new_file_state(&original_content, name, value);

    // ASSERT
    assert!(!result.contains("cdx="), "The complex ghost of cdx should be removed from the string");
    assert!(result.contains("ls=dir"), "Other lines should be preserved");
}

// alias_lib/src/lib.rs

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mesh_logic_merges_correctly() {
        let os = vec![("test".to_string(), "live".to_string())];
        let file = vec![("test".to_string(), "config".to_string())];

        let mesh = mesh_logic(os, file);

        assert_eq!(mesh.len(), 1);
        assert_eq!(mesh[0].os_value, Some("live".to_string()));
        assert_eq!(mesh[0].file_value, Some("config".to_string()));
    }

    #[test]
    fn test_beast_detection() {
        let entry = AliasEntryMesh {
            name: "cdx".to_string(),
            os_value: Some("".to_string()), // The Beast/Ghost
            file_value: Some("actual_path".to_string()),
        };
        assert!(entry.is_empty_definition());
    }
}

#[test]
fn test_sovereign_gatekeeper_logic() {
    // Harsh: No spaces, alphanumeric start
    assert!(is_valid_name("git123"));
    assert!(!is_valid_name(" git"));      // Leading space
    assert!(!is_valid_name("git "));      // Trailing space
    assert!(!is_valid_name("g s"));       // Internal space
    assert!(!is_valid_name("-git"));      // Symbol start
    assert!(!is_valid_name("\"git\""));   // Quote start

    // Loose: Spaces allowed later (for the Beast), alphanumeric start
    assert!(is_valid_name_loose("gs=git status"));
    assert!(!is_valid_name_loose(" gs=git"));      // Leading space still kills it
    assert!(!is_valid_name_loose("=git status"));  // Empty name kills it
}

#[test]
fn test_macro_file_ingest_resilience() {
    let content = "
        # Valid Comment
        ls=dir /w
        // Another Comment
        -bad=should be ignored
          leading_space=ignored
        valid_name = trimmed_value
        :: Old school comment
        [section]
        name with space=ignored
    ";

    let results: Vec<(String, String)> = content.lines()
        .map(|l| l.trim())
        .filter_map(|l| l.split_once('='))
        .filter(|(k, _)| is_valid_name(k.trim()))
        .map(|(k, v)| (k.trim().to_string(), v.trim().to_string()))
        .collect();

    // Now expecting 3 because we allow indented aliases!
    assert_eq!(results.len(), 3);
    assert_eq!(results[0].0, "ls");
    assert_eq!(results[1].0, "leading_space");
    assert_eq!(results[2].0, "valid_name");
}

#[test]
fn test_parser_beast_and_flags() {
    // Test: Volatile + Force + The Beast
    let args = vec!["alias".into(), "--temp".into(), "--force".into(), "gpp=g++ -std=c++20".into()];
    let (action, _) = parse_alias_args(&args);

    if let AliasAction::Set(opts) = action {
        assert_eq!(opts.name, "gpp");
        assert_eq!(opts.value, "g++ -std=c++20");
        assert!(opts.volatile);
        assert!(opts.force_case);
    } else {
        panic!("Failed to parse complex set action");
    }
}

#[test]
fn test_jason_prevention_on_cli() {
    // Attempting to inject JSON or Quoted strings
    let args = vec!["alias".into(), "{\"key\":\"val\"}".into()];
    let (action, _) = parse_alias_args(&args);
    assert_eq!(action, AliasAction::Invalid);

    let args2 = vec!["alias".into(), "\"ls\"=dir".into()];
    let (action2, _) = parse_alias_args(&args2);
    assert_eq!(action2, AliasAction::Invalid);
}

