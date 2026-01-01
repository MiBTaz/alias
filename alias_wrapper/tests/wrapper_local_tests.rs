use std::path::PathBuf;
use alias_lib::*;
use alias_wrapper::*;
use serial_test::serial;

pub fn get_test_path(suffix: &str) -> PathBuf {
    PathBuf::from(format!("test_{}_{:?}.doskey", suffix, std::thread::current().id()))
}

#[test]
#[serial]
fn a_nuke_the_world() {
    // This runs first (alphabetically) and calls your new lib
    alias_nuke::kernel_wipe_macros();
}

/* Cannot make this test reliable. works
   only if file name is not added (eg xyz works, xyz.rs doesn't
   as in it runs the tests regardless. but fails if named correctly
   feel free to epnd a few hour days weeks etc on the gordian knot
 */
/*
#[test]
#[serial]
fn test_wrapper_batch_purge_efficiency() {
    alias_nuke::kernel_wipe_macros();
    // 1. Seed
    set_alias(SetOptions::new("temp1", "val1"), &get_test_path("p1"), true).ok();
    set_alias(SetOptions::new("temp2", "val2"), &get_test_path("p2"), true).ok();

    // 2. Purge
    let _ = purge_ram_macros().expect("Batch purge failed");

    // 3. Give the kernel 50ms to breathe
    std::thread::sleep(std::time::Duration::from_millis(50));

    // 4. Verify
    let all = get_all_aliases();
    assert!(
        !all.iter().any(|(n, _)| n == "temp1" || n == "temp2"),
        "Macros persisted! Current RAM state: {:?}", all
    );
}

 */

#[test]
#[serial]
fn test_wrapper_transactional_disk_write() {
    let path = get_test_path("wrapper_transact");
    let opts = SetOptions {
        name: "disk_test".to_string(),
        value: "verified".to_string(),
        volatile: false,
        force_case: false,
    };

    // Execute the strike
    set_alias(opts, &path, true).expect("Wrapper strike failed");

    // Verify the file exists and the .tmp file is gone
    assert!(path.exists(), "Target file was not created");
    let tmp_path = path.with_extension("doskey.tmp");
    assert!(!tmp_path.exists(), "Leftover .tmp file found! Transaction incomplete.");
}

#[test]
#[serial]
fn test_wrapper_registry_append_preservation() {
    use winreg::RegKey;
    use winreg::enums::HKEY_CURRENT_USER;

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let (key, _) = hkcu.create_subkey(REG_SUBKEY).unwrap();

    // 1. Simulate an existing environment setup (e.g., Clink or Conda)
    let pre_existing = "echo 'Initializing Environment'";
    key.set_value("AutoRun", &pre_existing.to_string()).unwrap();

    // 2. Run the wrapper installer
    install_autorun(true).expect("Wrapper install failed");

    // 3. Verify the chain is preserved
    let final_val: String = key.get_value("AutoRun").unwrap();
    assert!(final_val.contains(pre_existing), "Pre-existing AutoRun command was deleted!");
    assert!(final_val.contains("--reload"), "Wrapper reload command missing!");
    assert!(final_val.contains(" & "), "Commands were not properly chained with '&'");
}

#[test]
#[serial]
fn test_wrapper_command_chain_escaping() {
    let path = get_test_path("wrapper_beast");
    let opts = SetOptions {
        name: "chain".to_string(),
        value: "echo part1 & echo part2 | findstr part".to_string(),
        volatile: false,
        force_case: false,
    };

    // Strike RAM and Disk via the wrapper
    set_alias(opts, &path, true).expect("Wrapper failed to set complex alias");

    // Query back to see if the pipe (|) and ampersand (&) survived
    let results = query_alias("chain", OutputMode::Normal);
    assert!(results.iter().any(|r| r.contains("echo part2")), "Command chain was truncated or mangled");
}

#[test]
#[serial]
fn z_nuke_the_world_end() {
    // This runs first (alphabetically) and calls your new lib
    alias_nuke::kernel_wipe_macros();
}
