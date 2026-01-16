// build.rs
use std::process::Command;

fn main() {
    // 1. Get the current Git Hash (Short)
    let output = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok();

    let git_hash = output
        .and_then(|out| String::from_utf8(out.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    // 2. Get the current timestamp
    let build_date = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();

    // 3. Emit these to the compiler
    println!("cargo:rustc-env=BUILD_REVISION={}", git_hash);
    println!("cargo:rustc-env=BUILD_DATE={}", build_date);

    // 4. Trigger re-run only if files change
    println!("cargo:rerun-if-changed=.git/HEAD");
}