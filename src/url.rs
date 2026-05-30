use std::io::Write;
use std::path::Path;

use anyhow::{Context, Result};

use crate::{READY_SENTINEL, URL_BANNER_PREFIX};

pub fn emit_banner(url: &str) -> Result<()> {
    let mut out = std::io::stdout().lock();
    writeln!(out, "{URL_BANNER_PREFIX}{url}").context("writing NATS_URL banner to stdout")?;
    writeln!(out, "{READY_SENTINEL}").context("writing ready sentinel to stdout")?;
    out.flush().context("flushing stdout")?;
    Ok(())
}

pub async fn write_url_file(path: &Path, url: &str) -> Result<()> {
    tokio::fs::write(path, format!("{url}\n"))
        .await
        .with_context(|| format!("writing URL file '{}'", path.display()))?;
    Ok(())
}

pub fn split_host_port(url: &str) -> Option<(String, u16)> {
    let bare = url.strip_prefix("nats://")?;
    let (host, port_str) = bare.rsplit_once(':')?;
    let port = port_str.parse().ok()?;
    Some((host.to_string(), port))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_host_port_works() {
        assert_eq!(
            split_host_port("nats://127.0.0.1:4222"),
            Some(("127.0.0.1".to_string(), 4222))
        );
    }
}
