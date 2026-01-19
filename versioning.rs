// versioning.rs
use std::process::Command;
use std::fs;
use std::path::Path;
use std::env;

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct Versioning {
    pub lib: &'static str,
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
    pub compile: u32,
    pub timestamp: &'static str,
}

pub fn create_versioning() {
    let out_dir = env::var("OUT_DIR").unwrap();
    let pkg_name = env::var("CARGO_PKG_NAME").unwrap();
    let major = env::var("CARGO_PKG_VERSION_MAJOR").unwrap_or_else(|_| "0".to_string());

    // 1. Resolve Physical Paths
    let current_dir = env::current_dir().unwrap();
    let folder_name = current_dir.file_name().unwrap().to_str().unwrap().to_string();

    let repo_root = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|_| ".".into());

    // --- THE HASH GATE ---
    // Get current HEAD hash
    let current_hash = Command::new("git")
        .current_dir(&repo_root)
        .args(["rev-parse", "HEAD"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|_| "unknown".into());

    let fingerprint_path = Path::new(&out_dir).join("last_git_hash.txt");
    let last_hash = fs::read_to_string(&fingerprint_path).unwrap_or_default();

    // If hash matches, the "Reality" hasn't changed. Exit now.
    if current_hash == last_hash && current_hash != "unknown" {
        return;
    }
    // ---------------------

    let git_pathspec = format!("{}/src/lib.rs", folder_name);

    // 2. Fetch Milestone Hashes
    let output = Command::new("git")
        .current_dir(&repo_root)
        .args([
            "log",
            "--format=%H",
            "--fixed-strings",
            "--grep=***",
            "--grep=*",
            "--",
            &git_pathspec
        ])
        .output()
        .expect("Failed to get git log");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut hashes: Vec<String> = stdout.lines().map(|s| s.to_string()).collect();

    let mut total_patch = 0;
    let mut total_minor = 0;
    let mut total_logic_churn = 0;

    if !hashes.is_empty() {
        let first_commit = Command::new("git")
            .current_dir(&repo_root)
            .args(["rev-list", "--max-parents=0", "HEAD"])
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .unwrap_or_default();

        if !hashes.contains(&first_commit) && !first_commit.is_empty() {
            hashes.push(first_commit);
        }

        // 3. Calculate Logic Deltas
        for window in hashes.windows(2) {
            let head = &window[0];
            let tail = &window[1];

            let diff = Command::new("git")
                .current_dir(&repo_root)
                .args(["diff", "-U0", tail, head, "--", &git_pathspec])
                .output().unwrap();

            let text = std::str::from_utf8(&diff.stdout).unwrap();
            let mut constructive_adds = 0;

            for line in text.lines() {
                if (line.starts_with('+') || line.starts_with('-')) && !line.starts_with("+++") && !line.starts_with("---") {
                    let content = line[1..].trim();
                    if content.is_empty() || content.starts_with("//") || content.starts_with("/*") || content.starts_with('*') {
                        continue;
                    }
                    total_logic_churn += 1;
                    if line.starts_with('+') {
                        constructive_adds += 1;
                    }
                }
            }
            if constructive_adds >= 100 { total_minor += 1; }
            else if constructive_adds >= 10 { total_patch += 1; }
        }
    }

    // 4. Baked Timestamp
    let timestamp = if cfg!(windows) {
        let output = Command::new("powershell").args(["-Command", "Get-Date -Format 'yyyy-MM-dd HH:mm'"]).output().unwrap();
        String::from_utf8_lossy(&output.stdout).trim().to_string()
    } else {
        let output = Command::new("date").arg("+%Y-%m-%d %H:%M").output().unwrap();
        String::from_utf8_lossy(&output.stdout).trim().to_string()
    };

    // 5. Generate Version Data Code
    let import = if pkg_name == "alias_lib" { "" } else { "use alias_lib::Versioning;" };
    let version_code = format!(
        r#"
    {}
    pub const VERSION: Versioning = Versioning {{
        lib: "{}",
        major: {},
        minor: {},
        patch: {},
        compile: {},
        timestamp: "{}",
    }};"#,
        import, pkg_name, major, total_minor, total_patch, total_logic_churn, timestamp
    );

    let dest_path = Path::new(&out_dir).join("version_data.rs");
    fs::write(&dest_path, version_code).unwrap();

    // Update fingerprint so we don't run again until the next commit
    fs::write(&fingerprint_path, current_hash).unwrap();

    // 6. Cargo Protocols
    // Use .git/HEAD for commit changes and src/lib.rs for local dirty edits
    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-changed=src/lib.rs");

    // FINAL AUDIT
    println!(
        "cargo:warning=[{}] Reality: v{}.{}.{} | Build Churn: {} | Baked: {}",
        pkg_name, major, total_minor, total_patch, total_logic_churn, timestamp
    );
}

