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

use riehub_vault::instance::{
    self as instance_coffer, InstanceCoffer, InstanceUnlocked, AUDIT_KEY_PATH, COOKIE_KEY_PATH,
};

/// The seal backend the config names.
pub fn seal_backend(config: &Config) -> Result<riehub_vault::instance::SealBackend> {
    riehub_vault::instance::SealBackend::from_config(
        &config.instance.seal,
        &config.instance.key_file,
    )
}

/// Opens the instance coffer with the configured seal backend.
pub async fn open_instance_coffer(
    config: &Config,
    store: &riehub_store::sqlite::SqliteStore,
) -> Result<(InstanceCoffer, InstanceUnlocked)> {
    let backend = seal_backend(config)?.host_seal()?;
    let unlocked = instance_coffer::open(store, backend.as_ref()).await?;
    let coffer = instance_coffer::load(store)
        .await?
        .ok_or_else(|| riehub_core::Error::NotFound("the instance coffer is gone".to_owned()))?;
    Ok((coffer, unlocked))
}

/// Loads and validates the config, then opens and migrates the store.
pub async fn open_store(config_path: Option<&Path>) -> Result<riehub_store::sqlite::SqliteStore> {
    let config = Config::load(config_path)?;
    config.validate()?;
    let store = riehub_store::open(&config.database.url).await?;
    store.migrate().await?;
    Ok(store)
}

/// The audit key, read from the instance coffer entry `audit/key`.
pub async fn load_audit_key(config: &Config) -> Result<Key32> {
    let store = riehub_store::open(&config.database.url).await?;
    let (coffer, unlocked) = open_instance_coffer(config, &store).await?;
    let bytes = instance_coffer::get_secret(&coffer.file, &unlocked, AUDIT_KEY_PATH)?;
    if bytes.len() != 32 {
        return Err(riehub_core::Error::Invalid(
            "the audit key entry must hold 32 bytes".to_owned(),
        ));
    }
    let mut key = [0u8; 32];
    key.copy_from_slice(&bytes);
    Ok(Key32::new(key))
}

/// `riehub init`: creates the data directory with mode 0700, runs the
/// migrations, creates the instance coffer with the audit and cookie
/// keys, and prints the armored instance recovery key exactly once
/// (api spec, section 4).
pub fn run_init(config_path: Option<&Path>) -> ExitCode {
    match init_with(config_path) {
        Ok(Some(armored)) => {
            print_line("riehub init ok");
            print_line("instance recovery key (shown once, store it safely):");
            print_line(&armored);
            ExitCode::SUCCESS
        }
        Ok(None) => {
            print_line("riehub init ok");
            ExitCode::SUCCESS
        }
        Err(e) => {
            print_error(&e.to_string());
            ExitCode::FAILURE
        }
    }
}

fn init_with(config_path: Option<&Path>) -> Result<Option<String>> {
    use std::os::unix::fs::PermissionsExt;
    let config = Config::load(config_path)?;
    config.validate()?;
    let dir = Path::new(&config.data.dir);
    match std::fs::metadata(dir) {
        Ok(meta) if meta.is_dir() => {}
        Ok(_) => {
            return Err(riehub_core::Error::Invalid(
                "data.dir: the path exists but is not a directory".to_owned(),
            ))
        }
        Err(_) => std::fs::create_dir(dir).map_err(|_| {
            riehub_core::Error::Invalid("data.dir: the directory is not creatable".to_owned())
        })?,
    }
    std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700)).map_err(|_| {
        riehub_core::Error::Invalid("data.dir: the mode is not settable".to_owned())
    })?;
    // The store and the coffer need the runtime for their async open.
    let rt = tokio::runtime::Runtime::new().map_err(|_| riehub_core::Error::Internal(None))?;
    rt.block_on(async move {
        let store = riehub_store::open(&config.database.url).await?;
        store.migrate().await?;
        if instance_coffer::load(&store).await?.is_some() {
            return Ok(None);
        }
        let backend = seal_backend(&config)?.host_seal()?;
        let (coffer, armored) = instance_coffer::create(&store, backend.as_ref()).await?;
        // The seal backend can always unseal what it just sealed, so
        // the first save re-opens the coffer through it.
        let unlocked = instance_coffer::open(&store, backend.as_ref()).await?;
        let entries = vec![
            (
                AUDIT_KEY_PATH.to_owned(),
                riehub_crypto::random_bytes::<32>().to_vec(),
            ),
            (
                COOKIE_KEY_PATH.to_owned(),
                riehub_crypto::random_bytes::<32>().to_vec(),
            ),
        ];
        instance_coffer::save(&store, &coffer, &unlocked, &entries, 1).await?;
        Ok(Some(armored))
    })
}

pub fn print_line(text: &str) {
    let mut out = std::io::stdout().lock();
    let _ = writeln!(out, "{text}");
}

pub fn print_error(text: &str) {
    let mut err = std::io::stderr().lock();
    let _ = writeln!(err, "{text}");
}

/// Prints an error and returns the failure exit code.
pub fn exit_with(e: &riehub_core::Error) -> ExitCode {
    print_error(&e.to_string());
    ExitCode::FAILURE
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
    let key = match load_audit_key(&config).await {
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
