use riehub_crypto::Key32;

fn main() {
    let key = Key32::new([0u8; 32]);
    println!("{:?}", key); // rules-lint: allow print - compile-fail fixture, never runs
}
