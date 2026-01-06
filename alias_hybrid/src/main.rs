// alias_hybrid/src/main.rs

use alias_lib::*;
use alias::HybridLibraryInterface as Interface;

fn main() {
    let args = std::env::args().collect();
    if let Err(e) = run::<Interface>(args) {
        // The Final Scream: main() is the only one allowed to
        // print a Percolated Error to stderr.
        eprintln!("{}", e);
        std::process::exit(1);
    }
}
