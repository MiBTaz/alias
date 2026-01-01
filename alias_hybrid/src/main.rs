// alias_hybrid/src/main.rs

use std::env;
use alias_lib::*;
// FIX: Point to the actual struct name you defined in lib.rs
use alias::HybridLibraryInterface as Interface;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args: Vec<String> = env::args().collect();
    inject_env_options(&mut args);

    let (action, quiet) = parse_alias_args(&args);
    let path = get_alias_path().ok_or("❌ Error: No alias file found.")?;

    // Now this will work because 'Interface' is 'HybridLibraryInterface'
    alias_lib::run::<Interface>(action, quiet, &path)
}