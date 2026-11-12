//! The `riehub` CLI: command parsing and the audit subcommands
//! (security spec, section 6). Exit codes: 0 ok, 1 error, 2 usage,
//! 3 verification failed.

use riehub_core::config::Config;
use riehub_core::error::Result;
use riehub_crypto::Key32;
use riehub_store::Store;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

/// The audit key file in the data directory, mode 0600, created at
/// first start. The instance coffer replaces this in T-0102.
#[derive(Debug)]
pub struct FileKeySource {
    path: PathBuf,
}

impl FileKeySource {
    pub fn new(data_dir: &str) -> Self {
        Self {
            path: Path::new(data_dir).join("audit.key"),
        }
    }

    fn ensure_file(&self) -> Result<()> {
        #[cfg(unix)]
        {
            use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
            match std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(&self.path)
            {
                Ok(mut file) => {
                    let key = riehub_crypto::random_bytes::<32>();
                    std::io::Write::write_all(&mut file, &key)
                        .map_err(|_| riehub_core::Error::Internal(None))?;
                    Ok(())
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                    let meta = std::fs::metadata(&self.path)
                        .map_err(|_| riehub_core::Error::Internal(None))?;
                    if meta.permissions().mode() & 0o077 != 0 {
                        return Err(riehub_core::Error::Invalid(
                            "audit key file: group or world readable files are refused".to_owned(),
                        ));
                    }
                    Ok(())
                }
                Err(_) => Err(riehub_core::Error::Invalid(
                    "audit key file: the data directory is not writable".to_owned(),
                )),
            }
        }
        #[cfg(not(unix))]
        {
            Err(riehub_core::Error::Invalid(
                "audit key file: this platform has no supported mode bits".to_owned(),
            ))
        }
    }
}

impl riehub_audit::KeySource for FileKeySource {
    fn load(&self) -> Result<Key32> {
        self.ensure_file()?;
        let bytes = std::fs::read(&self.path).map_err(|_| riehub_core::Error::Internal(None))?;
        if bytes.len() != 32 {
            return Err(riehub_core::Error::Invalid(
                "audit key file: it must hold exactly 32 bytes".to_owned(),
            ));
        }
        let mut key = [0u8; 32];
        key.copy_from_slice(&bytes);
        Ok(Key32::new(key))
    }
}

/// Loads and validates the config, then opens and migrates the store.
pub async fn open_store(config_path: Option<&Path>) -> Result<riehub_store::sqlite::SqliteStore> {
    let config = Config::load(config_path)?;
    config.validate()?;
    let store = riehub_store::open(&config.database.url).await?;
    store.migrate().await?;
    Ok(store)
}

pub fn print_line(text: &str) {
    let mut out = std::io::stdout().lock();
    let _ = writeln!(out, "{text}");
}

pub fn print_error(text: &str) {
    let mut err = std::io::stderr().lock();
    let _ = writeln!(err, "{text}");
}

#[derive(Debug, PartialEq, Eq)]
pub struct AuditArgs {
    pub command: String,
    pub config: Option<PathBuf>,
    pub from: u64,
}

#[allow(clippy::result_unit_err)]
pub fn parse_audit_args(args: &[String]) -> std::result::Result<AuditArgs, ()> {
    let mut sub = None;
    let mut config = None;
    let mut from = 1;
    let mut rest = args;
    while let Some((arg, tail)) = rest.split_first() {
        match arg.as_str() {
            "verify" | "head" | "export" if sub.is_none() => sub = Some(arg.clone()),
            "--config" => {
                let (value, more) = tail.split_first().ok_or(())?;
                config = Some(PathBuf::from(value));
                rest = more;
                continue;
            }
            "--from" => {
                let (value, more) = tail.split_first().ok_or(())?;
                from = value.parse().map_err(|_| ())?;
                rest = more;
                continue;
            }
            _ => return Err(()),
        }
        rest = tail;
    }
    match sub {
        Some(command) => Ok(AuditArgs {
            command,
            config,
            from,
        }),
        None => Err(()),
    }
}

/// The entry point for `riehub audit <sub>`. Returns the exit code.
pub async fn run_audit(args: &AuditArgs) -> ExitCode {
    let config = match Config::load(args.config.as_deref()) {
        Ok(c) => c,
        Err(e) => {
            print_error(&e.to_string());
            return ExitCode::FAILURE;
        }
    };
    if let Err(e) = config.validate() {
        print_error(&e.to_string());
        return ExitCode::FAILURE;
    }
    let store = match open_store(args.config.as_deref()).await {
        Ok(s) => s,
        Err(e) => {
            print_error(&e.to_string());
            return ExitCode::FAILURE;
        }
    };
    use riehub_audit::KeySource;
    let key_source = FileKeySource::new(&config.data.dir);
    let key = match key_source.load() {
        Ok(k) => k,
        Err(e) => {
            print_error(&e.to_string());
            return ExitCode::FAILURE;
        }
    };
    let mac = riehub_audit::AuditChainMac::new(key);

    match args.command.as_str() {
        "verify" => match riehub_audit::verify::verify(&store, &mac, args.from, None).await {
            Ok(outcome) => match outcome {
                riehub_audit::VerifyOutcome::Ok { head_seq } => {
                    print_line(&format!("audit chain ok through seq {head_seq}"));
                    ExitCode::SUCCESS
                }
                riehub_audit::VerifyOutcome::Broken { at, why } => {
                    print_line(&format!("audit chain broken at seq {at}: {why}"));
                    ExitCode::from(3)
                }
            },
            Err(e) => {
                print_error(&e.to_string());
                ExitCode::FAILURE
            }
        },
        "head" => match audit::head(&store).await {
            Ok(Some(head)) => {
                print_line(&format!(
                    "seq {} mac {}",
                    head.seq,
                    audit::to_hex(&head.mac)
                ));
                ExitCode::SUCCESS
            }
            Ok(None) => {
                print_line("audit log is empty");
                ExitCode::SUCCESS
            }
            Err(e) => {
                print_error(&e.to_string());
                ExitCode::FAILURE
            }
        },
        "export" => match audit::export(&store, args.from).await {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                print_error(&e.to_string());
                ExitCode::FAILURE
            }
        },
        _ => {
            print_error("unknown audit subcommand");
            ExitCode::from(2)
        }
    }
}

/// The store keeps its audit rows behind `Store`; the CLI readers walk
/// the rows in pages.
pub mod audit {
    use riehub_core::error::Result;
    use riehub_store::audit::AuditRow;
    use riehub_store::Store;

    pub async fn head<S: Store>(store: &S) -> Result<Option<AuditRow>> {
        let mut next = 1;
        let mut last = None;
        loop {
            let page = store
                .audit_read(next, 1000, &riehub_store::audit::AuditFilter::default())
                .await?;
            if page.is_empty() {
                break;
            }
            last = page.last().cloned();
            if page.len() < 1000 {
                break;
            }
            next = last.as_ref().map(|r| r.seq + 1).unwrap_or(next);
        }
        Ok(last)
    }

    pub async fn export<S: Store>(store: &S, from: u64) -> Result<()> {
        let rows = store
            .audit_read(from, u32::MAX, &riehub_store::audit::AuditFilter::default())
            .await?;
        for row in &rows {
            let event = serde_json::to_string(&row.event)
                .map_err(|_| riehub_core::Error::Internal(None))?;
            super::print_line(&format!(
                "{{\"seq\":{},\"ts\":{},\"kind\":\"{}\",\"event\":{},\"mac\":\"{}\"}}",
                row.seq,
                row.ts,
                row.kind,
                event,
                to_hex(&row.mac)
            ));
        }
        Ok(())
    }

    pub fn to_hex(bytes: &[u8]) -> String {
        let mut out = String::with_capacity(bytes.len() * 2);
        for b in bytes {
            out.push_str(&format!("{b:02x}"));
        }
        out
    }
}
