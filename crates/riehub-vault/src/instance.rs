//! The instance coffer (security spec, section 5): a second coffer
//! owned by the instance. Its vault key is wrapped by one host slot
//! whose key lives behind the configured seal backend. It holds the
//! audit key, the cookie key and shared secrets.

#![forbid(unsafe_code)]

use crate::map_integrity;
use cofferak_core::crypto::{
    decrypt_entry, encrypt_entry, generate_root_keypair, unwrap_dek_for_host, unwrap_dek_for_root,
    wrap_dek_for_host, wrap_dek_for_root, Aad, RootPrivateKey,
};
use cofferak_core::format::{VaultEntry, VaultFile};
use cofferak_core::seal::{HostSeal, SealError, SealedBlob, SystemdCredsSeal};
use riehub_core::clock::Clock;
use riehub_core::config::Seal;
use riehub_core::error::{Error, Result};
use riehub_core::{Owner, SystemClock};
use riehub_crypto::random_bytes;
use riehub_store::record::{Expect, Kind, Record};
use riehub_store::Store;
use std::path::PathBuf;
use zeroize::Zeroizing;

const VAULT_RECORD_ID: &str = "instance";
const HOST_KEY_RECORD_ID: &str = "instance.host-key";
pub const AUDIT_KEY_PATH: &str = "audit/key";
pub const COOKIE_KEY_PATH: &str = "cookie/key";

/// Where the host key lives, picked by `instance.seal`.
#[derive(Debug, Clone)]
pub enum SealBackend {
    /// Linux standalone, through `systemd-creds`.
    Systemd { name: String },
    /// macOS keychain.
    Keychain { name: String },
    /// A 32-byte key file with mode 0600, for containers.
    File { path: PathBuf },
}

impl SealBackend {
    pub fn from_config(seal: &Seal, key_file: &str) -> Result<Self> {
        match seal {
            Seal::Systemd => Ok(Self::Systemd {
                name: "riehub-instance".to_owned(),
            }),
            Seal::Keychain => Ok(Self::Keychain {
                name: "riehub-instance".to_owned(),
            }),
            Seal::File => {
                if key_file.is_empty() {
                    return Err(Error::Invalid(
                        "instance.seal: the file seal needs instance.key_file".to_owned(),
                    ));
                }
                Ok(Self::File {
                    path: PathBuf::from(key_file),
                })
            }
        }
    }

    /// The seal backend for this platform. `systemd` runs on Linux,
    /// `keychain` on macOS, `file` everywhere.
    pub fn host_seal(&self) -> Result<Box<dyn HostSeal + Send + Sync>> {
        match self {
            SealBackend::Systemd { name } => {
                #[cfg(target_os = "linux")]
                {
                    Ok(Box::new(SystemdCredsSeal { name: name.clone() }))
                }
                #[cfg(not(target_os = "linux"))]
                {
                    let _ = name;
                    Err(Error::Unavailable(
                        "the systemd creds seal runs on Linux only".to_owned(),
                    ))
                }
            }
            SealBackend::Keychain { name } => {
                #[cfg(target_os = "macos")]
                {
                    Ok(Box::new(cofferak_core::seal::KeychainSeal {
                        service: "riehub".to_owned(),
                        account: name.clone(),
                    }))
                }
                #[cfg(not(target_os = "macos"))]
                {
                    let _ = name;
                    Err(Error::Unavailable(
                        "the keychain seal runs on macOS only".to_owned(),
                    ))
                }
            }
            SealBackend::File { path } => Ok(Box::new(FileHostSeal { path: path.clone() })),
        }
    }
}

/// The file backend: a 32-byte key file with mode 0600. A file that is
/// group or world readable is refused on unseal.
#[derive(Debug)]
pub struct FileHostSeal {
    path: PathBuf,
}

impl HostSeal for FileHostSeal {
    fn seal(&self, key: &[u8; 32]) -> std::result::Result<SealedBlob, SealError> {
        #[cfg(unix)]
        {
            use std::io::Write as _;
            use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .mode(0o600)
                .open(&self.path)
                .map_err(SealError::Io)?;
            file.write_all(key).map_err(SealError::Io)?;
            std::fs::set_permissions(&self.path, std::fs::Permissions::from_mode(0o600))
                .map_err(SealError::Io)?;
            // The file backend stores the key in place, the blob is empty.
            Ok(SealedBlob(Vec::new()))
        }
        #[cfg(not(unix))]
        {
            let _ = key;
            Err(SealError::Invalid)
        }
    }

    fn unseal(&self, blob: &SealedBlob) -> std::result::Result<[u8; 32], SealError> {
        // The file backend stores the key in place, so its blob is
        // always empty. Anything else means the record was tampered
        // with and must fail closed.
        if !blob.0.is_empty() {
            return Err(SealError::Invalid(
                "the host key record does not match the file backend".to_owned(),
            ));
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let meta = std::fs::metadata(&self.path).map_err(SealError::Io)?;
            if meta.permissions().mode() & 0o077 != 0 {
                return Err(SealError::Invalid(
                    "the key file mode or size is wrong".to_owned(),
                ));
            }
            let bytes = std::fs::read(&self.path).map_err(SealError::Io)?;
            let mut key = [0u8; 32];
            if bytes.len() != 32 {
                return Err(SealError::Invalid(
                    "the key file mode or size is wrong".to_owned(),
                ));
            }
            key.copy_from_slice(&bytes);
            Ok(key)
        }
        #[cfg(not(unix))]
        {
            Err(SealError::Invalid)
        }
    }
}

/// The instance coffer, stored as instance records.
#[derive(Clone, Debug)]
pub struct InstanceCoffer {
    pub file: VaultFile,
}

/// An unlocked instance coffer. No `Debug`: the DEK is key material.
#[allow(missing_debug_implementations)]
pub struct InstanceUnlocked {
    dek: Zeroizing<[u8; 32]>,
    pub generation: u64,
}

impl InstanceUnlocked {
    pub fn expose(&self) -> &[u8; 32] {
        &self.dek
    }
}

/// Creates the instance coffer: new DEK, the host key behind the seal
/// backend, and both records written. Returns the coffer and the
/// armored recovery key, which the caller prints exactly once.
pub async fn create(store: &dyn Store, backend: &dyn HostSeal) -> Result<(InstanceCoffer, String)> {
    let host_key = random_bytes::<32>();
    let (root_pub, root_priv) = generate_root_keypair().map_err(|_| Error::Internal(None))?;
    let dek = random_bytes::<32>();
    let mut file = VaultFile::new(root_pub);
    file.hosts.insert(
        "instance".to_owned(),
        wrap_dek_for_host(&dek, "instance", &host_key),
    );
    file.root_wrap =
        Some(wrap_dek_for_root(&dek, &file.root_pubkey).map_err(|_| Error::Internal(None))?);
    let unlocked = InstanceUnlocked {
        dek: Zeroizing::new(dek),
        generation: 0,
    };
    file.seal(unlocked.expose());

    let sealed = backend.seal(&host_key).map_err(map_seal)?;
    let now = SystemClock.now_millis();
    let bytes = file.to_bytes().map_err(|_| Error::Internal(None))?;
    let vault_record = Record {
        owner: Owner::Instance,
        kind: Kind::Vault,
        id: VAULT_RECORD_ID.to_owned(),
        parent: None,
        lookup: None,
        status: None,
        sort: 0,
        meta: serde_json::json!({"generation": file.generation}),
        data: Some(bytes),
        version: 1,
        created_at: now,
        updated_at: now,
    };
    let key_record = Record {
        owner: Owner::Instance,
        kind: Kind::Vault,
        id: HOST_KEY_RECORD_ID.to_owned(),
        parent: None,
        lookup: None,
        status: None,
        sort: 0,
        meta: serde_json::json!({}),
        data: Some(sealed.0),
        version: 1,
        created_at: now,
        updated_at: now,
    };
    store.put(&vault_record, Expect::New).await?;
    store.put(&key_record, Expect::New).await?;

    let armored = root_priv.to_armored();
    Ok((InstanceCoffer { file }, armored.as_str().to_owned()))
}

fn map_seal(error: SealError) -> Error {
    match error {
        SealError::Unavailable => {
            Error::Unavailable("the host seal backend is not available on this host".to_owned())
        }
        _ => Error::Invalid("the host seal failed".to_owned()),
    }
}

/// Loads the instance coffer records without opening them.
pub async fn load(store: &dyn Store) -> Result<Option<InstanceCoffer>> {
    let record = store
        .get(&Owner::Instance, Kind::Vault, VAULT_RECORD_ID)
        .await?;
    match record {
        None => Ok(None),
        Some(record) => {
            let bytes = record.data.ok_or_else(|| Error::Internal(None))?;
            let file = VaultFile::from_bytes(&bytes).map_err(|_| Error::Internal(None))?;
            Ok(Some(InstanceCoffer { file }))
        }
    }
}

/// Opens the coffer: unseals the host key, unwraps the DEK and
/// verifies the file. Any failure here must stop the server (S6).
pub async fn open(store: &dyn Store, backend: &dyn HostSeal) -> Result<InstanceUnlocked> {
    let coffer = load(store).await?.ok_or_else(|| {
        Error::NotFound("the instance coffer does not exist, run riehub init".to_owned())
    })?;
    let key_record = store
        .get(&Owner::Instance, Kind::Vault, HOST_KEY_RECORD_ID)
        .await?
        .ok_or_else(|| Error::NotFound("the instance host key record is missing".to_owned()))?;
    let blob = SealedBlob(key_record.data.ok_or_else(|| Error::Internal(None))?);
    let host_key = backend.unseal(&blob).map_err(map_seal)?;
    let wrap = coffer
        .file
        .hosts
        .get("instance")
        .ok_or_else(|| Error::NotFound("the instance slot is missing".to_owned()))?;
    let dek = unwrap_dek_for_host(wrap, "instance", &host_key)
        .map_err(|_| Error::Unauthorized("the host key does not fit the coffer".to_owned()))?;
    coffer.file.verify(&dek, None).map_err(map_integrity)?;
    Ok(InstanceUnlocked {
        dek: Zeroizing::new(dek),
        generation: coffer.file.generation,
    })
}

/// Opens the coffer through the instance recovery key.
pub async fn recover(store: &dyn Store, recovery: &RootPrivateKey) -> Result<InstanceUnlocked> {
    let coffer = load(store)
        .await?
        .ok_or_else(|| Error::NotFound("the instance coffer does not exist".to_owned()))?;
    let wrap = coffer
        .file
        .root_wrap
        .as_ref()
        .ok_or_else(|| Error::NotFound("no root wrap".to_owned()))?;
    let dek = unwrap_dek_for_root(wrap, &coffer.file.root_pubkey, recovery)
        .map_err(|_| Error::Unauthorized("the recovery key does not fit".to_owned()))?;
    coffer.file.verify(&dek, None).map_err(map_integrity)?;
    Ok(InstanceUnlocked {
        dek: Zeroizing::new(dek),
        generation: coffer.file.generation,
    })
}

/// Reads the bytes at a path, decrypted under the DEK.
pub fn get_secret(file: &VaultFile, unlocked: &InstanceUnlocked, path: &str) -> Result<Vec<u8>> {
    let record = file
        .entries
        .get(path)
        .ok_or_else(|| Error::NotFound("no such coffer entry".to_owned()))?;
    decrypt_entry(unlocked.expose(), &Aad::entry(path), &record.ciphertext)
        .map_err(|_| Error::Internal(None))
}

/// Applies the entries, seals the file and writes the vault record
/// with compare-and-swap on the generation. The host key record is
/// written once at create and never changes. Instance coffer writes
/// carry no audit events: the audit key lives inside this coffer.
pub async fn save(
    store: &dyn Store,
    coffer: &InstanceCoffer,
    unlocked: &InstanceUnlocked,
    entries: &[(String, Vec<u8>)],
    expected_generation: u64,
) -> Result<u64> {
    let mut file = coffer.file.clone();
    for (path, bytes) in entries {
        let entry = encrypt_entry(unlocked.expose(), &Aad::entry(path), bytes);
        file.entries.insert(
            path.clone(),
            VaultEntry {
                ciphertext: entry,
                version: 1,
                updated_at: SystemClock.now_millis(),
            },
        );
    }
    file.seal(unlocked.expose());

    let now = SystemClock.now_millis();
    let bytes = file.to_bytes().map_err(|_| Error::Internal(None))?;
    let vault_record = Record {
        owner: Owner::Instance,
        kind: Kind::Vault,
        id: VAULT_RECORD_ID.to_owned(),
        parent: None,
        lookup: None,
        status: None,
        sort: 0,
        meta: serde_json::json!({"generation": file.generation}),
        data: Some(bytes),
        version: file.generation,
        created_at: now,
        updated_at: now,
    };
    let cas = if expected_generation == 0 {
        Expect::New
    } else {
        Expect::Version(expected_generation)
    };
    store.put(&vault_record, cas).await?;
    Ok(file.generation)
}
