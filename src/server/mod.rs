pub mod bundled;
pub mod healthz;

use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};

use crate::manifest::{AuthMode, Log, LogFormat, LogLevel, Manifest};

const READY_TIMEOUT: Duration = Duration::from_secs(5);
const DRAIN_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Debug, Clone)]
pub struct ServerOptions {
    pub bundled_strategy: bundled::Resolution,
    pub override_binary: Option<PathBuf>,
    pub host_override: Option<String>,
    pub port_override: Option<u16>,
    pub keep_storage: bool,
}

pub struct RunningServer {
    pub child: Child,
    pub host: String,
    pub port: u16,
    pub url: String,
    pub storage_dir: PathBuf,
    pub keep_storage: bool,
    pub config_path: PathBuf,
    pub log_stderr: bool,
    pub stderr_drain: Option<tokio::task::JoinHandle<()>>,
    pub stdout_drain: Option<tokio::task::JoinHandle<()>>,
}

impl RunningServer {
    pub async fn shutdown(mut self) -> Result<()> {
        if let Some(pid) = self.child.id() {
            #[cfg(unix)]
            {
                send_sigterm(pid as i32);
            }
            #[cfg(not(unix))]
            {
                let _ = pid;
            }
        }

        let wait_fut = self.child.wait();
        match tokio::time::timeout(DRAIN_TIMEOUT, wait_fut).await {
            Ok(Ok(_)) => {}
            Ok(Err(e)) => tracing::warn!("nats-server wait error: {e}"),
            Err(_) => {
                tracing::warn!("nats-server did not exit within drain timeout, killing");
                let _ = self.child.kill().await;
                let _ = self.child.wait().await;
            }
        }

        if let Some(h) = self.stderr_drain.take() {
            h.abort();
        }
        if let Some(h) = self.stdout_drain.take() {
            h.abort();
        }

        if !self.keep_storage {
            if let Err(e) = tokio::fs::remove_dir_all(&self.storage_dir).await {
                tracing::warn!(
                    "failed to remove storage dir {}: {}",
                    self.storage_dir.display(),
                    e
                );
            }
        } else {
            eprintln!(
                "hyerix-natsfixture: --keep-storage set; JetStream data at {} (remember to rm -rf when done)",
                self.storage_dir.display()
            );
        }
        let _ = tokio::fs::remove_file(&self.config_path).await;

        Ok(())
    }
}

pub async fn start(manifest: &Manifest, opts: &ServerOptions) -> Result<RunningServer> {
    let host = opts
        .host_override
        .clone()
        .unwrap_or_else(|| manifest.listen.host.clone());

    let mut port = opts.port_override.unwrap_or(manifest.listen.port);
    if port == 0 {
        port = pick_free_port(&host)?;
    }

    let binary = bundled::resolve(opts.override_binary.as_deref(), opts.bundled_strategy)
        .map_err(|e| anyhow!(e))?;

    let storage_dir = if manifest.storage.dir.is_empty() {
        let td = tempfile::Builder::new()
            .prefix("hyerix-natsfixture-")
            .tempdir()
            .context("creating JetStream storage tempdir")?;
        td.keep()
    } else {
        let p = PathBuf::from(&manifest.storage.dir);
        tokio::fs::create_dir_all(&p)
            .await
            .with_context(|| format!("creating storage dir '{}'", p.display()))?;
        p
    };

    let config = render_server_config(manifest, &host, port, &storage_dir);
    let config_path = storage_dir.join("nats-server.conf");
    tokio::fs::write(&config_path, &config)
        .await
        .with_context(|| format!("writing nats-server config to {}", config_path.display()))?;

    let (stdout_dest, stderr_dest) = log_destinations(&manifest.log);
    let mut cmd = Command::new(&binary);
    cmd.arg("-c").arg(&config_path);
    cmd.stdin(Stdio::null());
    cmd.stdout(stdout_dest);
    cmd.stderr(stderr_dest);

    let mut child = cmd
        .spawn()
        .with_context(|| format!("spawning nats-server at {}", binary.display()))?;

    let stderr_drain = child.stderr.take().map(|out| {
        tokio::spawn(async move {
            let mut reader = BufReader::new(out).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                eprintln!("[nats-server] {line}");
            }
        })
    });
    let stdout_drain = child.stdout.take().map(|out| {
        tokio::spawn(async move {
            let mut reader = BufReader::new(out).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                eprintln!("[nats-server] {line}");
            }
        })
    });

    healthz::wait_for_tcp(&host, port, READY_TIMEOUT)
        .await
        .map_err(|e| anyhow!(e))?;

    let url = format!("nats://{host}:{port}");
    Ok(RunningServer {
        child,
        host,
        port,
        url,
        storage_dir,
        keep_storage: opts.keep_storage,
        config_path,
        log_stderr: true,
        stderr_drain,
        stdout_drain,
    })
}

fn log_destinations(log: &Log) -> (Stdio, Stdio) {
    let _ = log;
    (Stdio::piped(), Stdio::piped())
}

fn pick_free_port(host: &str) -> Result<u16> {
    let listener = TcpListener::bind((host, 0))
        .with_context(|| format!("binding ephemeral port on {host}"))?;
    let port = listener
        .local_addr()
        .context("reading ephemeral port")?
        .port();
    drop(listener);
    Ok(port)
}

pub fn render_server_config(
    manifest: &Manifest,
    host: &str,
    port: u16,
    storage_dir: &Path,
) -> String {
    use std::fmt::Write;
    let mut s = String::new();
    let _ = writeln!(s, "server_name: hyerix-natsfixture");
    let _ = writeln!(s, "host: \"{host}\"");
    let _ = writeln!(s, "port: {port}");
    let _ = writeln!(s, "http: 0");

    let _ = writeln!(s, "jetstream {{");
    let _ = writeln!(s, "  store_dir: \"{}\"", storage_dir.display());
    let _ = writeln!(
        s,
        "  max_memory_store: {}",
        size_or_default(&manifest.storage.jetstream_max_memory, "1G")
    );
    let _ = writeln!(
        s,
        "  max_file_store: {}",
        size_or_default(&manifest.storage.jetstream_max_file, "10G")
    );
    let _ = writeln!(s, "}}");

    let level = match manifest.log.level {
        LogLevel::Off => "WARN",
        LogLevel::Error => "ERROR",
        LogLevel::Warn => "WARN",
        LogLevel::Info => "INFO",
        LogLevel::Debug => "DEBUG",
        LogLevel::Trace => "TRACE",
    };
    let _ = writeln!(s, "logtime: true");
    match manifest.log.level {
        LogLevel::Debug | LogLevel::Trace => {
            let _ = writeln!(s, "debug: true");
        }
        _ => {}
    }
    if matches!(manifest.log.level, LogLevel::Trace) {
        let _ = writeln!(s, "trace: true");
    }
    let _ = writeln!(s, "# log level (informational): {level}");
    if matches!(manifest.log.format, LogFormat::Json) {
        let _ = writeln!(
            s,
            "# (json log format requested; nats-server has no native JSON log; using text)"
        );
    }
    if !["stderr", "stdout", ""].contains(&manifest.log.destination.as_str()) {
        let _ = writeln!(s, "log_file: \"{}\"", manifest.log.destination);
    }

    match manifest.auth.mode {
        AuthMode::None => {}
        AuthMode::Token => {
            let _ = writeln!(s, "authorization {{");
            let _ = writeln!(s, "  token: \"{}\"", manifest.auth.token);
            let _ = writeln!(s, "}}");
        }
        AuthMode::UserPassword => {
            let _ = writeln!(s, "authorization {{");
            let _ = writeln!(s, "  users: [");
            for u in &manifest.auth.users {
                let _ = write!(
                    s,
                    "    {{ user: \"{}\", password: \"{}\"",
                    u.user, u.password
                );
                if let Some(perms) = &u.permissions {
                    let _ = write!(s, ", permissions: {{ ");
                    let pub_ = perms
                        .publish
                        .iter()
                        .map(|x| format!("\"{x}\""))
                        .collect::<Vec<_>>()
                        .join(", ");
                    let sub = perms
                        .subscribe
                        .iter()
                        .map(|x| format!("\"{x}\""))
                        .collect::<Vec<_>>()
                        .join(", ");
                    let _ = write!(s, "publish: [{pub_}], subscribe: [{sub}] }}");
                }
                let _ = writeln!(s, " }}");
            }
            let _ = writeln!(s, "  ]");
            let _ = writeln!(s, "}}");
        }
        AuthMode::Nkey | AuthMode::Jwt => {
            // validation rejects these in v0.1
        }
    }

    s
}

fn size_or_default<'a>(v: &'a str, default: &'a str) -> &'a str {
    if v.trim().is_empty() {
        default
    } else {
        v
    }
}

#[cfg(unix)]
fn send_sigterm(pid: i32) {
    extern "C" {
        fn kill(pid: i32, sig: i32) -> i32;
    }
    const SIGTERM: i32 = 15;
    unsafe {
        kill(pid, SIGTERM);
    }
}
