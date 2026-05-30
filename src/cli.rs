use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(
    name = "hyerix-natsfixture",
    version = crate::cli::full_version_string(),
    about = "Ephemeral NATS server fixture for tests. Spins up a manifest-defined NATS+JetStream, hands you a URL, cleans up on exit.",
    long_about = "hyerix-natsfixture is a single-binary CI fixture for NATS-dependent code. It boots an ephemeral nats-server (with JetStream / KV / Object Store pre-loaded from a YAML manifest), prints the connection URL, and cleans up on shutdown.\n\nDocs: https://github.com/hyerix/hyerix-natsfixture"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Run the fixture in the foreground until SIGINT/SIGTERM.
    Run(RunArgs),
    /// Parse and validate a manifest without starting the server. Exits 0 on success, 1 on failure.
    Validate(ValidateArgs),
    /// Run the fixture, exec a wrapped command with NATS_URL/HOST/PORT in its env, exit with the child's status.
    Exec(ExecArgs),
    /// Start the fixture in the background, print pid+URL, return immediately. Use `kill` to shut it down.
    Spawn(SpawnArgs),
    /// Terminate a fixture previously launched with `spawn`.
    Kill(KillArgs),
}

#[derive(Debug, clap::Args, Clone)]
pub struct CommonRunArgs {
    /// Path to the fixture manifest (YAML).
    #[arg(long)]
    pub manifest: PathBuf,

    /// Override the NATS listen port. 0 = OS-assigned (default if not given in manifest).
    #[arg(long)]
    pub port: Option<u16>,

    /// Override the NATS listen host.
    #[arg(long)]
    pub host: Option<String>,

    /// Path to a specific nats-server binary. Overrides the bundled one and PATH lookup.
    #[arg(long)]
    pub nats_server: Option<PathBuf>,

    /// Prefer a `nats-server` on PATH over the bundled binary.
    #[arg(long, default_value_t = false)]
    pub prefer_system_nats_server: bool,

    /// Write the resolved nats:// URL to this file after startup.
    #[arg(long)]
    pub url_file: Option<PathBuf>,

    /// Keep the JetStream storage directory after shutdown (for post-mortem debugging).
    #[arg(long, default_value_t = false)]
    pub keep_storage: bool,

    /// Suppress all fixture-level logs except errors and the URL banner.
    #[arg(long, default_value_t = false)]
    pub quiet: bool,
}

#[derive(Debug, clap::Args)]
pub struct RunArgs {
    #[command(flatten)]
    pub common: CommonRunArgs,
}

#[derive(Debug, clap::Args)]
pub struct ValidateArgs {
    /// Path to the fixture manifest (YAML).
    #[arg(long)]
    pub manifest: PathBuf,
}

#[derive(Debug, clap::Args)]
pub struct ExecArgs {
    #[command(flatten)]
    pub common: CommonRunArgs,

    /// Command and arguments to run with NATS_URL/NATS_HOST/NATS_PORT in env.
    #[arg(last = true, required = true)]
    pub argv: Vec<String>,
}

#[derive(Debug, clap::Args)]
pub struct SpawnArgs {
    #[command(flatten)]
    pub common: CommonRunArgs,

    /// File to which the spawned child's PID is written.
    #[arg(long)]
    pub pid_file: PathBuf,
}

#[derive(Debug, clap::Args)]
pub struct KillArgs {
    /// File containing the PID of a previously-spawned fixture.
    #[arg(long)]
    pub pid_file: PathBuf,
}

pub fn full_version_string() -> &'static str {
    static CACHE: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    CACHE
        .get_or_init(|| {
            let nats = crate::server::bundled::detect_nats_server_version()
                .unwrap_or_else(|| "unknown".to_string());
            format!(
                "{} (bundled nats-server: {})",
                env!("CARGO_PKG_VERSION"),
                nats
            )
        })
        .as_str()
}
