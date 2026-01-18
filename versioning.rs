use std::process::Command;
use std::fs;
use std::path::Path;

pub fn create_versioning() { // Must be pub
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let out_dir = std::env::var("OUT_DIR").unwrap();
    let lib_rs = format!("{}/src/lib.rs", manifest_dir);
    let pkg_name = std::env::var("CARGO_PKG_NAME").unwrap();

    // 1. Get Milestone Hashes (Commits with '*')
    let output = Command::new("git")
        .args(["log", "--format=%H", "--grep=\\*", "--", &lib_rs])
        .output()
        .expect("Failed to get git log");

    let hashes: Vec<&str> = std::str::from_utf8(&output.stdout).unwrap().lines().collect();

    let mut total_patch = 0;
    let mut total_minor = 0;
    let mut total_logic_churn = 0;

    // 2. Process Deltas between Milestones
    if hashes.len() >= 2 {
        for i in 0..hashes.len() - 1 {
            let head = hashes[i];
            let tail = hashes[i+1];

            let diff = Command::new("git")
                .args(["diff", "-U0", tail, head, "--", &lib_rs])
                .output()
                .expect("Failed diff");

            let text = std::str::from_utf8(&diff.stdout).unwrap();
            let mut milestone_weight = 0;

            for line in text.lines() {
                if (line.starts_with('+') || line.starts_with('-'))
                    && !line.starts_with("+++") && !line.starts_with("---") {

                    let content = line[1..].trim();
                    // FILTER: Ignore comments and empty lines
                    if content.is_empty() || content.starts_with("//") ||
                        content.starts_with("/*") || content.starts_with('*') {
                        continue;
                    }
                    milestone_weight += 1;
                }
            }

            if milestone_weight >= 100 { total_minor += 1; }
            else if milestone_weight >= 10 { total_patch += 1; }
            total_logic_churn += milestone_weight;
        }
    }

    let import = if pkg_name == "alias_lib" { "" } else { "use alias_lib::Versioning;" };

    let version_code = format!(
        r#"
        {}
        pub const VERSION: Versioning = Versioning {{
            lib: "{}",
            major: 0,
            minor: {},
            patch: {},
            compile: {},
            timestamp: "{}",
        }};"#,
        import,
        pkg_name,
        total_minor, total_patch, total_logic_churn,
        "2026-01-08 14:20" // Or use chrono
    );

    let dest_path = Path::new(&out_dir).join("version_data.rs");
    fs::write(&dest_path, version_code).unwrap();

    println!("cargo:rerun-if-changed=.git/index");
    println!("cargo:rerun-if-changed=src/lib.rs");
    println!("cargo:warning=LIB: {} | PATCH: {} | LOGIC CHURN: {}", pkg_name, total_patch, total_logic_churn);
}