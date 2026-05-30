//! Rust integration test pattern using hyerix-natsfixture.
//!
//! Drop this in `tests/integration.rs` of any crate that talks to NATS.
//! The `Fixture` struct boots the fixture in spawn mode and tears it down
//! via Drop, so individual tests just call `Fixture::start()` and use `f.url`.
//!
//! Alternative: just wrap `cargo test` with `hyerix-natsfixture exec`:
//!
//!     hyerix-natsfixture exec --manifest fixture.yaml -- cargo test
//!
//! That's simpler if you don't need per-test fixtures.

use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

pub struct Fixture {
    child: Child,
    pid_file: tempfile::NamedTempFile,
    url_file: tempfile::NamedTempFile,
    pub url: String,
}

impl Fixture {
    pub fn start(manifest: &str) -> std::io::Result<Self> {
        let pid_file = tempfile::NamedTempFile::new()?;
        let url_file = tempfile::NamedTempFile::new()?;

        let child = Command::new("hyerix-natsfixture")
            .arg("spawn")
            .arg("--manifest")
            .arg(manifest)
            .arg("--pid-file")
            .arg(pid_file.path())
            .arg("--url-file")
            .arg(url_file.path())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .spawn()?;

        // Wait for the URL file to be populated.
        let deadline = Instant::now() + Duration::from_secs(5);
        while std::fs::metadata(url_file.path()).map(|m| m.len()).unwrap_or(0) == 0 {
            if Instant::now() > deadline {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    "hyerix-natsfixture did not start within 5s",
                ));
            }
            std::thread::sleep(Duration::from_millis(50));
        }

        let url = std::fs::read_to_string(url_file.path())?.trim().to_string();
        Ok(Self { child, pid_file, url_file, url })
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        // Best-effort: tell the fixture to shut down cleanly, then wait briefly.
        let _ = Command::new("hyerix-natsfixture")
            .arg("kill")
            .arg("--pid-file")
            .arg(self.pid_file.path())
            .status();
        let _ = self.child.wait();
    }
}

// --- Example test using the fixture ---

#[tokio::test]
async fn publish_then_subscribe_roundtrip() {
    let f = Fixture::start("fixture.yaml").expect("fixture should start");
    let client = async_nats::connect(&f.url).await.unwrap();

    let mut sub = client.subscribe("test.subject".into()).await.unwrap();
    client.publish("test.subject".into(), "hello".into()).await.unwrap();
    client.flush().await.unwrap();

    let msg = sub.next().await.unwrap();
    assert_eq!(&msg.payload[..], b"hello");
}
