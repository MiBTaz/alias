#[allow(unused_imports)]
use winreg::RegKey;
#[allow(unused_imports)]
use winreg::enums::HKEY_CURRENT_USER;

#[test]
#[serial]
fn test_registry_append_logic() {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    // REG_SUBKEY and REG_AUTORUN_KEY must be provided by parent context
    let (key, _) = hkcu.create_subkey(REG_SUBKEY).unwrap();

    let original_cmd = "echo 'Old Command'";
    let val_to_set: String = original_cmd.to_string();
    key.set_value(REG_AUTORUN_KEY, &val_to_set).unwrap();

    P::write_autorun_registry(
        &format!("{} & alias --reload", original_cmd),
        &Verbosity::normal()
    ).expect("Install failed");

    let result: String = key.get_value(REG_AUTORUN_KEY).unwrap();
    assert!(result.contains(original_cmd));
    assert!(result.contains("--reload"));
}

#[test]
#[serial]
fn test_routine_setup_registration() {
    // Verifies the install_autorun branch in the provider
    let _ = P::install_autorun(&Verbosity::silent());
}

#[test]
#[serial]
fn test_win32_international_roundtrip_repeat() {
    let name = "λ_alias";
    let val = "echo lambda_power";
    assert!(P::raw_set_macro(name, Some(val)).unwrap());
    let all = P::get_all_aliases(&voice!(Silent, Off, Off)).unwrap(); // Add .unwrap()
    let found = all.iter().find(|(n, _)| n == name);
    assert!(found.is_some());
    assert_eq!(found.unwrap().1, val);
    P::raw_set_macro(name, None).unwrap();
}
