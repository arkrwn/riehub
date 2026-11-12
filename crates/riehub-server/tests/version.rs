// Runs the real binary, so unwrapping its exit is fine here.
#![allow(clippy::unwrap_used)]

use std::process::Command;

#[test]
fn version_prints_workspace_version() {
    let out = Command::new(env!("CARGO_BIN_EXE_riehub")).output().unwrap();
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(stdout.starts_with("riehub 0.0.1"), "stdout was: {stdout}");
}
