//! Prints the manifest the host reads: `cargo run -p chromalab --example manifest`.
fn main() {
    println!("{}", chromalab::manifest());
}
