use riehub_core::Secret;

fn main() {
    let secret = Secret::new(String::from("hunter2"));
    let _ = serde_json::to_string(&secret);
}
