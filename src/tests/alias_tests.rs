// src/tests/alias_tests.rs

use crate::*;
use std::env;
use std::fs;

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
        (AliasAction::Set {
            name: "gs".to_string(),
            value: "git status".to_string()
        }, false)
    );
}

#[test]
fn test_set_with_space() {
    let args = vec!["alias".to_string(), "vi".to_string(), "nvim".to_string(), "-o".to_string()];
    assert_eq!(
        parse_alias_args(&args),
        (AliasAction::Set {
            name: "vi".to_string(),
            value: "nvim -o".to_string()
        }, false)
    );
}

#[test]
fn test_delete_syntax() {
    let args = vec!["alias".to_string(), "junk=".to_string()];
    assert_eq!(
        parse_alias_args(&args),
        (AliasAction::Set {
            name: "junk".to_string(),
            value: "".to_string()
        }, false)
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
    assert_eq!(action1, AliasAction::Set { name: "gs".into(), value: "git status".into() });
}

#[test]
fn test_set_complex_value_with_spaces() {
    let args = vec!["alias".into(), "my_cmd".into(), "echo".into(), "hi".into()];
    let (action, _) = parse_alias_args(&args);
    if let AliasAction::Set { name, value } = action {
        assert_eq!(name, "my_cmd");
        assert_eq!(value, "echo hi");
    } else {
        panic!("Set failed");
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
fn test_set_with_multiple_equals() {
    // Testing: alias logic="a=b" (The first equals should be the split point)
    let args = vec!["alias".into(), "logic=a=b".into()];
    let (action, _) = parse_alias_args(&args);
    assert_eq!(action, AliasAction::Set {
        name: "logic".into(),
        value: "a=b".into()
    });
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

    args.splice(1..1, extra_opts);

    let (action, quiet) = parse_alias_args(&args);
    assert!(quiet);
    if let AliasAction::Set { name, value } = action {
        assert_eq!(name, "g");
        assert_eq!(value, "git status");
    } else {
        panic!("Failed to parse Set action after injection");
    }
}

#[test]
fn test_empty_value_is_delete() {
    let args = vec!["alias".to_string(), "old_alias=".to_string()];
    let (action, _) = parse_alias_args(&args);
    if let AliasAction::Set { name, value } = action {
        assert_eq!(name, "old_alias");
        assert_eq!(value, ""); // Should be interpreted as a deletion
    }
}

#[test]
fn test_space_separator_set() {
    // Testing the "alias name value" alternate syntax
    let args = vec!["alias".to_string(), "ll".to_string(), "ls".to_string(), "-la".to_string()];
    let (action, _) = parse_alias_args(&args);
    if let AliasAction::Set { name, value } = action {
        assert_eq!(name, "ll");
        assert_eq!(value, "ls -la");
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
    // 1. Simulate the initial args from the OS (alias set g=status)
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

    if let AliasAction::Set { name, value } = action {
        assert_eq!(name, "g");
        assert_eq!(value, "status");
    } else {
        panic!("Action should have been AliasAction::Set");
    }
}

#[test]
fn test_quiet_extraction() {
    let args = vec!["alias".to_string(), "--quiet".to_string(), "g=status".to_string()];
    let (action, quiet) = parse_alias_args(&args);

    assert!(quiet); // The bool is caught
    assert_eq!(action, AliasAction::Set { name: "g".into(), value: "status".into() }); // Action is clean
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

    // We create the vector but keep it as a separate binding.
    // This way, 'input' stays alive in the scope of the test.
    let args = vec!["alias".to_string(), input];

    // We pass a reference to the vector's contents.
    let action = parse_alias_args(&args).0;

    if let AliasAction::Set { name: n, value: v } = action {
        assert_eq!(n, name);
        assert_eq!(v, value);
    } else {
        // Now 'args[1]' is used here, and it's perfectly valid
        // because 'args' hasn't been dropped or moved yet.
        panic!("Parser failed to identify '{}' as a Set/Delete action", args[1]);
    }
}

#[test]
fn test_real_file_deletion() {
    // 1. Create a dummy file with an alias
    let path = PathBuf::from("test_aliases.txt");
    fs::write(&path, "cdx=some_old_command\n").unwrap();

    // 2. Run the logic that should delete it
    let name = "cdx";
    let value = ""; // The delete signal
    set_alias(name, value, &path, true).unwrap();

    // 3. Read the file back
    let content = fs::read_to_string(&path).unwrap();

    // 4. THIS should have failed in the old code if the file wasn't updating
    assert!(!content.contains("cdx="), "The file should not contain the deleted alias!");

    // Cleanup
    fs::remove_file(path).unwrap();
}

#[test]
fn test_alias_deletion_persistence() {
    let name = "cdx";
    let value = "";
    let test_path = PathBuf::from("ghost_test.doskey");

    // 1. Create a file with the alias in it
    fs::write(&test_path, "cdx=FOR /F tokens=* %i IN ('v:\\lbin\\ncd.exe $*') DO @(set OLDPWD=%CD% & chdir /d %i)\n").unwrap();

    // 2. RUN the actual set_alias function (This uses the variables!)
    let _ = set_alias(name, value, &test_path, true);

    // 3. READ it back
    let content = fs::read_to_string(&test_path).unwrap();

    // 4. ASSERT - This will fail if your filter logic is broken
    assert!(!content.contains("cdx="), "The ghost of cdx is still in the file!");

    // Cleanup
    let _ = fs::remove_file(test_path);
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
        AliasAction::Set { name, value } => {
            assert_eq!(name, "rust");
            assert_eq!(value, "cargo");
        }
        _ => panic!("Expected Set action"),
    }
}

#[test]
fn test_parse_set_space() {
    let args = vec!["alias".to_string(), "rust".to_string(), "cargo".to_string()];
    let (action, _) = parse_alias_args(&args);
    match action {
        AliasAction::Set { name, value } => {
            assert_eq!(name, "rust");
            assert_eq!(value, "cargo");
        }
        _ => panic!("Expected Set action"),
    }
}

#[test]
fn test_quiet_flag_positioning() {
    let args = vec!["alias".to_string(), "--quiet".to_string(), "ls=dir".to_string()];
    let (_, quiet) = parse_alias_args(&args);
    assert!(quiet);
}

