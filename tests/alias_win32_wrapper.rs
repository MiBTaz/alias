use std::fs;
use std::path::PathBuf;
use crate::*;

#[test]
fn test_real_file_deletion() {
    // 1. Create a dummy file with an alias
    let path = PathBuf::from(format!("test_aliases_{:?}.txt", std::thread::current().id()));
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
    let test_path = PathBuf::from(format!("ghost_test_{:?}.doskey", std::thread::current().id()));

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

