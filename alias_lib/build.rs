// alias_lib/build.rs

use std::env;

#[path = "../versioning.rs"]
mod versioning;

fn main() {
    versioning::create_versioning();
    // 1. Get Workspace Root (One level up from alias_lib)
    let current_dir = env::current_dir().unwrap();
    let workspace_root = current_dir.parent().expect("Must be in a workspace");
    let repo_root_str = workspace_root.to_str().unwrap();

    // 2. AGGREGATE: Generate the GLOBAL file inside alias_lib/src
    // Writing to src/ makes it a real, trackable part of the crate.
    let dest_path = current_dir.join("src").join("generated_overall.rs");

    let targets = [
        ("alias_lib", "alias_lib", 0),
        ("alias_win32", "alias_win32", 0),
        ("alias_wrapper", "alias_wrapper", 0),
        ("alias_hybrid", "alias", 0),
    ];

    let mut total_minor = 0;
    let mut total_patch = 0;
    let mut total_churn = 0;
    let mut first_ts = String::from("unknown");

    let mut code = String::from("// AUTO-GENERATED - DO NOT EDIT\n\n");

    for (folder, pkg, maj) in targets {
        // Use the brain to get the data
        let v = versioning::calculate_reality(repo_root_str, folder, pkg, maj);

        total_minor += v.minor;
        total_patch += v.patch;
        total_churn += v.compile;
        if first_ts == "unknown" { first_ts = v.timestamp.clone(); }

        let const_name = format!("VER_{}", pkg.to_uppercase().replace("-", "_"));
        code.push_str(&format!(
            "pub const {}: Versioning = Versioning {{ lib: \"{}\", major: {}, minor: {}, patch: {}, compile: {}, timestamp: \"{}\" }};\n",
            const_name, v.pkg_name, v.major, v.minor, v.patch, v.compile, v.timestamp
        ));
    }

    code.push_str(&format!(
        "\npub const SYSTEM_REALITY: Versioning = Versioning {{ lib: \"WORKSPACE\", major: 0, minor: {}, patch: {}, compile: {}, timestamp: \"{}\" }};\n",
        total_minor, total_patch, total_churn, first_ts
    ));

    std::fs::write(&dest_path, code).expect("Failed to write generated_overall.rs");

    // 3. LOCAL: Also run the standard version_data.rs for the lib's internal use
    versioning::create_versioning();

    // RERUN TRIGGERS
    println!("cargo:rerun-if-changed=../versioning.rs");
    for (folder, _, _) in targets {
        println!("cargo:rerun-if-changed=../{}/src/lib.rs", folder);
    }
}
