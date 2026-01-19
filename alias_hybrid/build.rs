// alias_hybrid/build.rs

#[path = "../versioning.rs"]
mod versioning;

fn main() {
    versioning::create_versioning();
}