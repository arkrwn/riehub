//! riehub: One private hub for every AI model, tool, skill and workflow.

#![forbid(unsafe_code)]

pub mod cli;

use riehub_core::Config;
use std::io::Write as _;
use std::path::PathBuf;
use std::process::ExitCode;

/// The line `riehub version` prints.
pub fn version_line() -> String {
    format!("riehub {}", env!("CARGO_PKG_VERSION"))
}

enum Command {
    Version,
    Serve { config: Option<PathBuf> },
    Doctor,
    Audit(cli::AuditArgs),
}

fn usage() -> ExitCode {
    let mut err = std::io::stderr().lock();
    let _ = writeln!(
        err,
        "usage: riehub [version] | serve [--config <path>] | doctor | audit verify|head|export"
    );
    ExitCode::from(2)
}

fn parse_args(args: &[String]) -> Result<Command, ()> {
    match args.first().map(String::as_str) {
        None => Ok(Command::Version),
        Some("version") => Ok(Command::Version),
        Some("serve") => {
            let mut config = None;
            let mut rest = &args[1..];
            while let Some((flag, tail)) = rest.split_first() {
                match flag.as_str() {
                    "--config" => {
                        let (value, more) = tail.split_first().ok_or(())?;
                        config = Some(PathBuf::from(value));
                        rest = more;
                    }
                    _ => return Err(()),
                }
            }
            Ok(Command::Serve { config })
        }
        Some("doctor") => {
            if args.len() != 1 {
                return Err(());
            }
            Ok(Command::Doctor)
        }
        Some("audit") => Ok(Command::Audit(cli::parse_audit_args(&args[1..])?)),
        Some(_) => Err(()),
    }
}

fn main() -> ExitCode {
    // print_stdout is denied workspace-wide, so output goes through
    // io::Write instead of println!.
    let args: Vec<String> = std::env::args().skip(1).collect();
    let command = match parse_args(&args) {
        Ok(command) => command,
        Err(()) => return usage(),
    };

    match command {
        Command::Version => {
            let mut out = std::io::stdout().lock();
            let ok = writeln!(out, "{}", version_line()).is_ok();
            if ok {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            }
        }
        Command::Serve { config } => serve(config),
        Command::Doctor => doctor(),
        Command::Audit(args) => {
            let rt = match tokio::runtime::Runtime::new() {
                Ok(rt) => rt,
                Err(_) => return ExitCode::FAILURE,
            };
            rt.block_on(cli::run_audit(&args))
        }
    }
}

fn serve(config_path: Option<PathBuf>) -> ExitCode {
    let config = match Config::load(config_path.as_deref()) {
        Ok(config) => config,
        Err(e) => return report(&e, 1),
    };
    if let Err(e) = config.validate() {
        return report(&e, 1);
    }
    let mut out = std::io::stdout().lock();
    let _ = writeln!(out, "config ok (profile: {})", config.profile.as_str());
    ExitCode::SUCCESS
}

fn doctor() -> ExitCode {
    let mut all_ok = true;
    let mut out = std::io::stdout().lock();

    let config = match Config::load(None) {
        Ok(config) => config,
        Err(e) => {
            let _ = writeln!(out, "config: fail ({e})");
            return ExitCode::from(3);
        }
    };

    match config.validate() {
        Ok(()) => {
            let _ = writeln!(out, "config: ok");
        }
        Err(e) => {
            let _ = writeln!(out, "config: fail ({e})");
            all_ok = false;
        }
    }

    let (data_ok, data_note) = check_data_dir(&config);
    report_check(&mut out, "data-dir", data_ok, &data_note, &mut all_ok);

    let (seal_ok, seal_note) = check_seal(&config);
    report_check(&mut out, "seal", seal_ok, &seal_note, &mut all_ok);

    let (clock_ok, clock_note) = check_clock();
    report_check(&mut out, "clock", clock_ok, &clock_note, &mut all_ok);

    let (tls_ok, tls_note) = check_tls(&config);
    report_check(&mut out, "tls", tls_ok, &tls_note, &mut all_ok);

    if all_ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(3)
    }
}

fn report_check(
    out: &mut impl std::io::Write,
    name: &str,
    ok: bool,
    note: &str,
    all_ok: &mut bool,
) {
    if ok {
        let _ = writeln!(out, "{name}: ok");
    } else {
        let _ = writeln!(out, "{name}: fail ({note})");
        *all_ok = false;
    }
}

fn check_data_dir(config: &Config) -> (bool, String) {
    let path = &config.data.dir;
    match std::fs::metadata(path) {
        Ok(meta) if meta.is_dir() => {
            if world_readable(path) {
                (
                    false,
                    "the data directory is readable by other users".to_owned(),
                )
            } else {
                (true, String::new())
            }
        }
        Ok(_) => (false, format!("{path} is not a directory")),
        Err(_) => (false, format!("{path} does not exist, run riehub init")),
    }
}

#[cfg(unix)]
fn world_readable(path: &str) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path)
        .map(|m| m.permissions().mode() & 0o004 != 0)
        .unwrap_or(false)
}

#[cfg(not(unix))]
fn world_readable(_path: &str) -> bool {
    false
}

fn check_seal(config: &Config) -> (bool, String) {
    if config.instance.seal == riehub_core::Seal::File && config.instance.key_file.is_empty() {
        (
            false,
            "instance.seal is file but instance.key_file is empty".to_owned(),
        )
    } else {
        (true, String::new())
    }
}

fn check_clock() -> (bool, String) {
    use riehub_core::{Clock, SystemClock};
    // 2020-01-01T00:00:00Z in UTC milliseconds.
    if SystemClock.now_millis() >= 1_577_836_800_000 {
        (true, String::new())
    } else {
        (false, "the system clock looks wrong".to_owned())
    }
}

fn check_tls(config: &Config) -> (bool, String) {
    use riehub_core::TlsMode;
    if config.server.tls.mode == TlsMode::Off {
        return (true, String::new());
    }
    let cert = &config.server.tls.cert;
    let key = &config.server.tls.key;
    if cert.is_empty() || key.is_empty() {
        return (
            false,
            "tls mode is files but cert or key is empty".to_owned(),
        );
    }
    for path in [cert.as_str(), key.as_str()] {
        if !std::path::Path::new(path).is_file() {
            return (false, format!("{path} is not a file"));
        }
    }
    if key_file_mode_open(key) {
        return (false, "the tls key file is readable by others".to_owned());
    }
    (true, String::new())
}

#[cfg(unix)]
fn key_file_mode_open(path: &str) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path)
        .map(|m| m.permissions().mode() & 0o077 != 0)
        .unwrap_or(true)
}

#[cfg(not(unix))]
fn key_file_mode_open(_path: &str) -> bool {
    false
}

fn report(e: &riehub_core::Error, code: u8) -> ExitCode {
    let mut err = std::io::stderr().lock();
    let _ = writeln!(err, "{e}");
    ExitCode::from(code)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn greeting_names_the_project() {
        assert!(version_line().starts_with("riehub"));
    }
}
