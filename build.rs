// build.rs
use std::process::Command;

#[path = "tests/state_restoration.rs"]
mod stateful;
#[path = "versioning.rs"] // Or wherever you have it centralized
mod versioning;
use versioning::Versioning;

fn main() {
    println!("cargo:rerun-if-changed=alias_hybrid\\src\\lib.rs");
    println!("cargo:rerun-if-changed=alias_lib\\src\\lib.rs");
    println!("cargo:rerun-if-changed=alias_lib\\tests\\library_tests.rs");
    println!("cargo:rerun-if-changed=alias_win32\\src\\lib.rs");
    println!("cargo:rerun-if-changed=alias_wrapper\\src\\lib.rs");
    println!("cargo:rerun-if-changed=../alias_lib/src");
    println!("cargo:rerun-if-changed=../alias_win32/src");
    println!("cargo:rerun-if-changed=../alias_wrapper/src");

    // 1. BOOTSTRAP: Write a valid file IMMEDIATELY if it's empty or missing
    // This allows the following 'cargo check' to actually pass.
    seed_overall_if_needed();

    // 2. GENERATE: Force sub-projects to create their individual version_data.rs
    let _ = Command::new("cargo")
        .args(["check", "-p", "alias_lib", "-p", "alias_win32", "-p", "alias_wrapper", "-p", "alias_hybrid"])
        .status();

    // 3. AGGREGATE: Now that the data exists, scrape it and overwrite the seed
    main_overall_aggregate();

    main_vars();
    main_state();
}

fn main_vars() {
    let output = Command::new("git").args(["rev-parse", "--short", "HEAD"]).output().ok();
    let git_hash = output
        .and_then(|out| String::from_utf8(out.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    let build_date = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();

    println!("cargo:rustc-env=BUILD_REVISION={}", git_hash);
    println!("cargo:rustc-env=BUILD_DATE={}", build_date);
    println!("cargo:rerun-if-changed=build.rs");
}
fn main_state() {
    if std::env::var("PROFILE").unwrap_or_default() == "debug" {
        // Respect the Mutex used by the tests
        let _lock = stateful::GlobalNamedMutex::acquire();

        let hkcu = winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER);
        // Only overwrite if no tests are currently active
        let current_count: u32 = hkcu.open_subkey(BACKUP_KEY_PATH)
            .and_then(|k| k.get_value("ActiveCount")).unwrap_or(0);

        if current_count == 0 {
            if let Ok((key, _)) = hkcu.create_subkey(BACKUP_KEY_PATH) {
                // 1. CAPTURE Golden Image (Vec<String> for MULTI_SZ)
                let aliases = raw_get_all_aliases_as_strings();
                let _ = key.set_value("Aliases", &aliases);

                // 2. CAPTURE AutoRun
                let run_path = r"Software\Microsoft\Command Processor";
                let autorun: String = hkcu.open_subkey(run_path)
                    .and_then(|k| k.get_value("AutoRun")).unwrap_or_default();
                let _ = key.set_value("AutoRun", &autorun);

                // 3. CLEAN ROOM setup
                stateful::clear_all_macros(); // Targeted delete
                if let Ok(k) = hkcu.open_subkey_with_flags(run_path, winreg::enums::KEY_SET_VALUE) {
                    let _ = k.set_value("AutoRun", &"");
                }

                // Initialized and ready for tests
                let _ = key.set_value("ActiveCount", &0u32);
            }
        }
    }
}

// Helper for build.rs to get strings exactly how winreg wants them
fn raw_get_all_aliases_as_strings() -> Vec<String> {
    let mut buffer = vec![0u16; 65536];
    unsafe {
        let res = windows_sys::Win32::System::Console::GetConsoleAliasesW(
            buffer.as_mut_ptr(), 131072, "cmd.exe\0".encode_utf16().collect::<Vec<_>>().as_ptr() as *mut _
        );
        if res == 0 { return vec![]; }
        let char_count = (res as usize + 1) / 2;
        String::from_utf16_lossy(&buffer[..char_count])
            .split('\0')
            .filter(|s| !s.is_empty() && s.contains('='))
            .map(|s| s.to_string())
            .collect()
    }
}

pub fn raw_get_all_aliases() -> Vec<(String, String)> {
    // Calling the Win32 API directly to get current RAM aliases
    // This is a simplified version of what's in your win32_lib
    let mut buffer = [0u16; 32768];
    let exe_name: Vec<u16> = "cmd.exe\0".encode_utf16().collect();

    unsafe {
        let len = windows_sys::Win32::System::Console::GetConsoleAliasesW(
            buffer.as_mut_ptr(),
            buffer.len() as u32 * 2,
            exe_name.as_ptr() as *mut _
        );
        if len == 0 { return vec![]; }

        String::from_utf16_lossy(&buffer[..len as usize / 2])
            .split('\0')
            .filter(|s| !s.is_empty())
            .filter_map(|s| s.split_once('='))
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }
}

pub fn raw_nuke_aliases() {
    let exe_name: Vec<u16> = "cmd.exe\0".encode_utf16().collect();
    unsafe {
        // Passing null as the source buffer nukes all aliases for that target
        windows_sys::Win32::System::Console::AddConsoleAliasW(
            ptr::null_mut(),
            ptr::null_mut(),
            exe_name.as_ptr() as *mut _
        );
    }
}

fn main_overall_aggregate() {
    let root = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    // Corrected list based on your 'dir' output
    let members = ["alias_lib", "alias_win32", "alias_wrapper", "alias"];
    let dest_path = std::path::Path::new(&root).join("generated_overall.rs");

    let mut total_minor = 0;
    let mut total_patch = 0;
    let mut total_churn = 0;
    let mut timestamps = Vec::new();

    println!("cargo:warning=--- Aggregator Trace ---");
    println!("cargo:warning=Targeting: {}", dest_path.display());

    for member in members {
        if let Some(path) = find_member_version_file(member) {
            println!("cargo:warning=[FOUND] {} -> {}", member, path.display());
            if let Ok(content) = std::fs::read_to_string(&path) {
                let m = scrape_val(&content, "minor");
                let p = scrape_val(&content, "patch");
                let c = scrape_val(&content, "compile");
                total_minor += m;
                total_patch += p;
                total_churn += c;
                if let Some(ts) = scrape_str(&content, "timestamp") {
                    timestamps.push(ts);
                }
                println!("cargo:warning=      Data: minor={}, patch={}, churn={}", m, p, c);
            }
        } else {
            println!("cargo:warning=[MISS]  Could not find version_data for {}", member);
        }
    }

    let overall_code = format!(
        r#"pub const SYSTEM_REALITY: Versioning = Versioning {{
    lib: "WORKSPACE",
    major: {},
    minor: {},
    patch: {},
    compile: {},
    timestamp: "{}",
}};"#,
        std::env::var("CARGO_PKG_VERSION_MAJOR").unwrap_or_else(|_| "0".into()),
        total_minor, total_patch, total_churn,
        timestamps.first().unwrap_or(&"unknown".to_string())
    );

    println!("cargo:warning=Writing WORKSPACE reality to file...");
    std::fs::write(&dest_path, overall_code).unwrap();
    println!("cargo:warning=--- Trace End ---");
}

fn seed_overall_if_needed() {
    let root = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let dest_path = std::path::Path::new(&root).join("generated_overall.rs");

    // If file is missing OR size is 0 (from a touch), write the minimal valid structure
    if !dest_path.exists() || std::fs::metadata(&dest_path).map(|m| m.len()).unwrap_or(0) == 0 {
        let seed = r#"// Auto-generated Seed
pub const SYSTEM_REALITY: Versioning = Versioning {
    lib: "BOOTSTRAP", major: 0, minor: 0, patch: 0, compile: 0, timestamp: "initial"
};"#;
        std::fs::write(&dest_path, seed).unwrap();
    }
}

fn find_member_version_file(member: &str) -> Option<std::path::PathBuf> {
    // PROFILE is set by Cargo (debug vs release)
    let profile = std::env::var("PROFILE").unwrap_or_else(|_| "debug".into());
    let root = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let build_dir = std::path::Path::new(&root).join("target").join(&profile).join("build");

    if let Ok(entries) = std::fs::read_dir(&build_dir) {
        for entry in entries.filter_map(|e| e.ok()) {
            let name = entry.file_name().to_string_lossy().to_string();
            // Cargo folders are member-hash (e.g., alias_lib-cc64ffd11038272e)
            if name == member || name.starts_with(&format!("{}-", member)) {
                let p = entry.path().join("out").join("version_data.rs");
                if p.exists() {
                    return Some(p);
                }
            }
        }
    }
    None
}

fn scrape_val(content: &str, key: &str) -> u32 {
    let pattern = format!("{}:", key);
    content.lines()
        .find(|l| l.contains(&pattern))
        .and_then(|l| l.split(':').nth(1))
        .and_then(|v| v.trim().trim_matches(',').parse().ok())
        .unwrap_or(0)
}

fn scrape_str(content: &str, key: &str) -> Option<String> {
    let pattern = format!("{}:", key);
    content.lines()
        .find(|l| l.contains(&pattern))
        .and_then(|l| l.split('"').nth(1))
        .map(|s| s.to_string())
}





