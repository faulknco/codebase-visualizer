use serde::Deserialize;
use std::path::{Path, PathBuf};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::net::UnixListener;
use tokio::sync::mpsc;

#[derive(Debug, Clone, Deserialize)]
pub struct ActivityEvent {
    pub file: String,
    pub action: String,
    pub agent: String,
    pub timestamp: u64,
}

/// Compute a deterministic socket path for a repo.
pub fn socket_path(repo_path: &Path) -> PathBuf {
    let canonical = repo_path.canonicalize().unwrap_or_else(|_| repo_path.to_path_buf());
    let hash: u32 = canonical.to_string_lossy().bytes()
        .fold(0u32, |acc, b| acc.wrapping_mul(31).wrapping_add(b as u32));
    PathBuf::from(format!("/tmp/cviz-{:08x}.sock", hash))
}

pub struct SocketGuard {
    path: PathBuf,
}

impl Drop for SocketGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

pub async fn run(repo_path: PathBuf, tx: mpsc::Sender<ActivityEvent>) -> SocketGuard {
    let sock_path = socket_path(&repo_path);

    // Remove stale socket
    let _ = std::fs::remove_file(&sock_path);

    let guard = SocketGuard { path: sock_path.clone() };

    let listener = match UnixListener::bind(&sock_path) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("[cviz] Failed to bind socket {}: {}", sock_path.display(), e);
            return guard;
        }
    };

    eprintln!("[cviz] Activity socket: {}", sock_path.display());

    loop {
        match listener.accept().await {
            Ok((stream, _)) => {
                let tx = tx.clone();
                tokio::spawn(async move {
                    let reader = BufReader::new(stream);
                    let mut lines = reader.lines();
                    while let Ok(Some(line)) = lines.next_line().await {
                        match serde_json::from_str::<ActivityEvent>(&line) {
                            Ok(event) => {
                                let _ = tx.send(event).await;
                            }
                            Err(e) => {
                                eprintln!("[cviz] Invalid activity event: {}", e);
                            }
                        }
                    }
                });
            }
            Err(e) => {
                eprintln!("[cviz] Socket accept error: {}", e);
            }
        }
    }
}
