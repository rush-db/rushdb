//! Boot-time listener: the HTTP port answers from process start (C6,
//! cloud-capacity F6).
//!
//! Opening a large store means redb open or repair, WAL replay and the
//! derived-index rebuild, which take tens of seconds. Before this module the
//! listener was bound only afterwards, so an orchestrator probing `/health`
//! saw "connection refused" and could restart the process in a loop.
//! [`BootListener`] binds the configured HTTP address first and answers on it
//! until the node is ready:
//! - `/health` → 200 `{"status":"starting","phase":…}` (the process is alive);
//! - `/ready` → 503 with the current [`phase`];
//! - anything else → 503 with the protocol `unavailable` error (retryable)
//!   and `Retry-After: 1`.
//!
//! [`BootListener::finish`] stops answering and hands the **same** socket to
//! the real router, so there is no window in which the port is closed.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Ordered boot phases reported by `/ready` while the node starts.
pub const PHASES: [&str; 4] = ["starting", "opening-storage", "starting-engines", "ready"];

static PHASE: AtomicUsize = AtomicUsize::new(0);

/// Record the current boot phase (an index into [`PHASES`]).
pub fn set_phase(phase: &str) {
    if let Some(index) = PHASES.iter().position(|known| *known == phase) {
        PHASE.store(index, Ordering::Release);
    }
}

/// The current boot phase name.
#[must_use]
pub fn phase() -> &'static str {
    PHASES[PHASE.load(Ordering::Acquire).min(PHASES.len() - 1)]
}

/// A pre-bound HTTP listener that answers health/readiness probes while the
/// node boots.
pub struct BootListener {
    stop: Arc<AtomicBool>,
    worker: std::thread::JoinHandle<TcpListener>,
}

impl BootListener {
    /// Bind `addr:port` and start answering probes.
    ///
    /// # Errors
    /// Returns a message when the address cannot be bound.
    pub fn start(addr: &str, port: u16) -> Result<Self, String> {
        let listener = TcpListener::bind((addr, port)).map_err(|e| e.to_string())?;
        listener.set_nonblocking(true).map_err(|e| e.to_string())?;
        let stop = Arc::new(AtomicBool::new(false));
        let started = Instant::now();
        let worker = {
            let stop = Arc::clone(&stop);
            std::thread::Builder::new()
                .name("rushdb-boot-listener".into())
                .spawn(move || {
                    while !stop.load(Ordering::Acquire) {
                        match listener.accept() {
                            Ok((stream, _)) => answer(stream, started),
                            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                                std::thread::sleep(Duration::from_millis(20));
                            }
                            Err(error) => {
                                tracing::warn!(%error, "boot listener accept failed");
                                std::thread::sleep(Duration::from_millis(20));
                            }
                        }
                    }
                    listener
                })
                .map_err(|e| e.to_string())?
        };
        Ok(Self { stop, worker })
    }

    /// Stop answering and return the bound socket for the real router.
    ///
    /// # Errors
    /// Returns a message if the boot listener thread panicked.
    pub fn finish(self) -> Result<TcpListener, String> {
        self.stop.store(true, Ordering::Release);
        self.worker
            .join()
            .map_err(|_| "boot listener thread panicked".to_string())
    }
}

fn answer(mut stream: std::net::TcpStream, started: Instant) {
    let _ = stream.set_nonblocking(false);
    let _ = stream.set_read_timeout(Some(Duration::from_secs(1)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(1)));
    // Only the request line matters, but read the whole (bounded) header
    // block: a request can arrive in several segments, and closing the
    // socket with unread bytes resets the connection instead of delivering
    // the response.
    let mut buf = [0u8; 2048];
    let mut n = 0;
    while n < buf.len() && !buf[..n].windows(4).any(|w| w == b"\r\n\r\n") {
        match stream.read(&mut buf[n..]) {
            Ok(0) | Err(_) => break,
            Ok(read) => n += read,
        }
    }
    let line = std::str::from_utf8(&buf[..n])
        .unwrap_or("")
        .lines()
        .next()
        .unwrap_or("");
    let path = line.split_whitespace().nth(1).unwrap_or("");
    let phase = phase();
    let elapsed_ms = started.elapsed().as_millis();
    let (status, body) = match path {
        "/health" => (
            "200 OK",
            format!(r#"{{"status":"starting","phase":"{phase}","elapsedMs":{elapsed_ms}}}"#),
        ),
        "/ready" => (
            "503 Service Unavailable",
            format!(r#"{{"status":"starting","phase":"{phase}","elapsedMs":{elapsed_ms}}}"#),
        ),
        _ => (
            "503 Service Unavailable",
            format!(
                r#"{{"protocolVersion":1,"error":{{"code":"unavailable","message":"node is starting ({phase})","retryable":true}}}}"#
            ),
        ),
    };
    let response = format!(
        "HTTP/1.1 {status}\r\ncontent-type: application/json\r\nretry-after: 1\r\n\
         content-length: {}\r\nconnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.write_all(response.as_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;

    fn get(port: u16, path: &str) -> String {
        let mut stream = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
        write!(stream, "GET {path} HTTP/1.1\r\nhost: x\r\n\r\n").unwrap();
        let mut out = String::new();
        stream.read_to_string(&mut out).unwrap();
        out
    }

    #[test]
    fn answers_probes_while_booting_and_hands_over_the_same_socket() {
        let boot = BootListener::start("127.0.0.1", 0).unwrap();
        // Port 0: discover the assigned port through a clone of the socket.
        let port = {
            let listener = boot.finish().unwrap();
            let port = listener.local_addr().unwrap().port();
            drop(listener);
            port
        };
        let boot = BootListener::start("127.0.0.1", port).unwrap();
        let health = get(port, "/health");
        assert!(health.starts_with("HTTP/1.1 200"), "{health}");
        // Other tests boot nodes concurrently and move the process-wide
        // phase, so only the field's presence is asserted here.
        assert!(health.contains(r#""phase":""#), "{health}");
        let ready = get(port, "/ready");
        assert!(ready.starts_with("HTTP/1.1 503"), "{ready}");
        let execute = get(port, "/protocol/v1/execute");
        assert!(execute.starts_with("HTTP/1.1 503"), "{execute}");
        assert!(execute.contains(r#""retryable":true"#), "{execute}");
        let listener = boot.finish().unwrap();
        assert_eq!(listener.local_addr().unwrap().port(), port);
    }
}
