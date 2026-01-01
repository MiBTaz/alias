use std::env;
use alias_lib::*;
// Swap this based on the crate:
use alias_win32::Win32LibraryInterface as Interface;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Collect args and merge %ALIAS_OPTS%
    let mut args: Vec<String> = env::args().collect();
    if let Ok(opts) = env::var(ENV_ALIAS_OPTS) {
        let extra: Vec<String> = opts.split_whitespace().map(String::from).collect();
        args.splice(1..1, extra);
    }

    // 2. Parse intent and find file
    let (action, quiet) = parse_alias_args(&args);
    let path = get_alias_path().ok_or("❌ Error: No alias file found.")?;

    // 3. Static Handoff
    run::<Interface>(action, quiet, &path)
}