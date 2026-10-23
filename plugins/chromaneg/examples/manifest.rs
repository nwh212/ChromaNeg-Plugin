//! Prints the manifest the host reads: `cargo run -p chromaneg --example manifest`.
fn main() {
    println!("{}", chromaneg::manifest());
}
