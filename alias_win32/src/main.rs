// alias_win32/src/main.rs

use alias_lib::*;
// Swap this based on the crate:
use alias_win32::Win32LibraryInterface as Interface;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    run::<Interface>(std::env::args().collect())
}
