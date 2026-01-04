// alias_wrapper/src/main.rs

use alias_lib::*;
// Swap this based on the crate:
use alias_wrapper::WrapperLibraryInterface as Interface;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    run::<Interface>(std::env::args().collect())
}
