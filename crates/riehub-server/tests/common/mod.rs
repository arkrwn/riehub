//! Shared helpers for end-to-end server tests: boot the real binary,
//! capture its output, and scan for privacy canaries (security spec,
//! section 8).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::io::{Read, Write as _};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub struct TestServer {
    pub port: u16,
    pub data_dir: PathBuf,
    child: Child,
    stdout: Arc<Mutex<String>>,
    stderr: Arc<Mutex<String>>,
}

/// Boots the real `riehub` binary on a temp data directory and a free
/// loopback port with log level trace. Stops the process on drop.
impl TestServer {
    pub fn start() -> Self {
        let dir = std::env::temp_dir().join(format!(
            "riehub-e2e-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();

        // A free loopback port: bind, read the port, release.
        let probe = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = probe.local_addr().unwrap().port();
        drop(probe);

        let url = format!("sqlite://{}/riehub.db", dir.display());
        let key_file = dir.join("host.key");
        let config_path = dir.join("riehub.toml");
        std::fs::write(
            &config_path,
            format!(
                "profile = \"dev\"\n\n[server]\nbind = \"127.0.0.1:{port}\"\n\n[data]\ndir = \"{}\"\n\n[database]\nurl = \"{url}\"\n\n[instance]\nseal = \"file\"\nkey_file = \"{}\"\n\n[log]\nlevel = \"trace\"\n",
                dir.display(),
                key_file.display()
            ),
        )
        .unwrap();

        // The instance coffer must exist before serve can start.
        let init = Command::new(env!("CARGO_BIN_EXE_riehub"))
            .arg("init")
            .arg("--config")
            .arg(&config_path)
            .output()
            .unwrap();
        if !init.status.success() {
            panic!(
                "riehub init failed: {}",
                String::from_utf8_lossy(&init.stderr)
            );
        }

        let mut child = Command::new(env!("CARGO_BIN_EXE_riehub"))
            .arg("serve")
            .arg("--config")
            .arg(&config_path)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();

        let stdout = Arc::new(Mutex::new(String::new()));
        let stderr = Arc::new(Mutex::new(String::new()));
        let mut stdout_pipe = child.stdout.take().unwrap();
        let mut stderr_pipe = child.stderr.take().unwrap();
        let stdout_cap = stdout.clone();
        let stderr_cap = stderr.clone();
        std::thread::spawn(move || {
            let mut buf = String::new();
            let _ = stdout_pipe.read_to_string(&mut buf);
            *stdout_cap.lock().unwrap() = buf;
        });
        std::thread::spawn(move || {
            let mut buf = String::new();
            let _ = stderr_pipe.read_to_string(&mut buf);
            *stderr_cap.lock().unwrap() = buf;
        });

        let server = Self {
            port,
            data_dir: dir,
            child,
            stdout,
            stderr,
        };
        server.wait_ready();
        server
    }

    fn wait_ready(&self) {
        let deadline = std::time::Instant::now() + Duration::from_secs(15);
        loop {
            if std::time::Instant::now() > deadline {
                let stderr = self.stderr.lock().unwrap().clone();
                panic!("the server did not come up in time. stderr: {stderr}");
            }
            if let Ok((status, _)) = self.request("GET", "/healthz", &[], None) {
                if status == 200 {
                    return;
                }
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    /// One HTTP request over a raw connection. Returns (status, body).
    pub fn request(
        &self,
        method: &str,
        target: &str,
        headers: &[(&str, &str)],
        body: Option<&str>,
    ) -> std::io::Result<(u16, String)> {
        let mut stream = TcpStream::connect(("127.0.0.1", self.port))?;
        let mut text =
            format!("{method} {target} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n");
        for (name, value) in headers {
            text.push_str(&format!("{name}: {value}\r\n"));
        }
        if let Some(body) = body {
            text.push_str(&format!("content-length: {}\r\n", body.len()));
        }
        text.push_str("\r\n");
        stream.write_all(text.as_bytes())?;
        if let Some(body) = body {
            stream.write_all(body.as_bytes())?;
        }
        let mut raw = Vec::new();
        stream.read_to_end(&mut raw)?;
        let text = String::from_utf8_lossy(&raw).to_string();
        let status: u16 = text
            .lines()
            .next()
            .and_then(|line| line.split_whitespace().nth(1))
            .and_then(|code| code.parse().ok())
            .unwrap_or(0);
        Ok((status, text))
    }

    /// Searches every file under the data directory and the captured
    /// output for the canary, raw and as base64 and hex.
    pub fn scan_for(&self, canary: &str) -> Vec<Hit> {
        let needles = [
            ("raw", canary.as_bytes().to_vec()),
            ("base64", base64(canary.as_bytes())),
            ("hex", hex_encode(canary.as_bytes())),
        ];
        let mut hits = Vec::new();
        scan_dir(&self.data_dir, &needles, &mut hits);
        for (name, text) in [("stdout", &self.stdout), ("stderr", &self.stderr)] {
            let buffer = text.lock().unwrap();
            for (encoding, needle) in &needles {
                if let Some(pos) = find_subslice(buffer.as_bytes(), needle) {
                    hits.push(Hit {
                        where_: format!("{name} at byte {pos}"),
                        encoding: encoding.to_string(),
                    });
                }
            }
        }
        hits
    }
}

impl TestServer {
    /// Stops the process. The captured output and the data directory
    /// stay readable for the scanner.
    pub fn stop(&mut self) {
        if self.child.try_wait().ok().flatten().is_none() {
            let _ = self.child.kill();
        }
        let _ = self.child.wait();
    }
}

impl Drop for TestServer {
    fn drop(&mut self) {
        if self.child.try_wait().ok().flatten().is_none() {
            let _ = self.child.kill();
        }
        let _ = self.child.wait();
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct Hit {
    pub where_: String,
    pub encoding: String,
}

fn scan_dir(dir: &Path, needles: &[(&str, Vec<u8>)], hits: &mut Vec<Hit>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            scan_dir(&path, needles, hits);
            continue;
        }
        let bytes = match std::fs::read(&path) {
            Ok(bytes) => bytes,
            Err(_) => continue,
        };
        for (encoding, needle) in needles {
            if let Some(pos) = find_subslice(&bytes, needle) {
                hits.push(Hit {
                    where_: format!("{} at byte {}", path.display(), pos),
                    encoding: encoding.to_string(),
                });
            }
        }
    }
}

fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return None;
    }
    (0..=haystack.len() - needle.len()).find(|i| &haystack[*i..*i + needle.len()] == needle)
}

fn hex_encode(bytes: &[u8]) -> Vec<u8> {
    bytes
        .iter()
        .flat_map(|b| {
            let s = format!("{b:02x}");
            s.into_bytes()
        })
        .collect()
}

fn base64(bytes: &[u8]) -> Vec<u8> {
    const ALPHA: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = Vec::new();
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = chunk.get(1).map(|b| *b as u32).unwrap_or(0);
        let b2 = chunk.get(2).map(|b| *b as u32).unwrap_or(0);
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(ALPHA[((n >> 18) & 63) as usize]);
        out.push(ALPHA[((n >> 12) & 63) as usize]);
        if chunk.len() > 1 {
            out.push(ALPHA[((n >> 6) & 63) as usize]);
        } else {
            out.push(b'=');
        }
        if chunk.len() > 2 {
            out.push(ALPHA[(n & 63) as usize]);
        } else {
            out.push(b'=');
        }
    }
    out
}
