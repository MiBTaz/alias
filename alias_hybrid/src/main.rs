// alias_hybrid/src/main.rs

use alias_lib::*;
// FIX: Point to the actual struct name you defined in lib.rs
use alias::HybridLibraryInterface as Interface;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    run::<Interface>(std::env::args().collect())
}
