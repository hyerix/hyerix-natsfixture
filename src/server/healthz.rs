use std::time::{Duration, Instant};

use tokio::net::TcpStream;
use tokio::time::sleep;

pub async fn wait_for_tcp(host: &str, port: u16, total: Duration) -> Result<(), HealthzError> {
    let deadline = Instant::now() + total;
    loop {
        match TcpStream::connect((host, port)).await {
            Ok(_) => return Ok(()),
            Err(_) => {
                if Instant::now() >= deadline {
                    return Err(HealthzError::Timeout {
                        host: host.to_string(),
                        port,
                        waited: total,
                    });
                }
                sleep(Duration::from_millis(25)).await;
            }
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum HealthzError {
    #[error("nats-server did not accept TCP connections on {host}:{port} within {waited:?}")]
    Timeout {
        host: String,
        port: u16,
        waited: Duration,
    },
}
