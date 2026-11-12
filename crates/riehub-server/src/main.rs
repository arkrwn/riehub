//! riehub: One private hub for every AI model, tool, skill and workflow.

use std::io::Write as _;
use std::process::ExitCode;

fn main() -> ExitCode {
    // print_stdout is denied workspace-wide, so the version goes out
    // through io::Write instead of println!.
    let mut out = std::io::stdout().lock();
    let ok = writeln!(out, "{}", riehub_server::version_line()).is_ok();
    if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}
