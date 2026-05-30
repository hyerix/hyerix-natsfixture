use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::Parser;
use tracing_subscriber::EnvFilter;

use hyerix_natsfixture::{
    cli::{Cli, Command, CommonRunArgs},
    exec, exit_codes,
    manifest::{self, ManifestError},
    server::{self, bundled::Resolution, ServerOptions},
    url,
};

#[tokio::main]
async fn main() -> ExitCode {
    let cli = Cli::parse();

    init_tracing(quiet_flag(&cli));

    let code = match dispatch(cli).await {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("{e:#}");
            exit_codes::SERVER_FAILED
        }
    };
    ExitCode::from(code as u8)
}

fn quiet_flag(cli: &Cli) -> bool {
    match &cli.command {
        Command::Run(r) => r.common.quiet,
        Command::Exec(e) => e.common.quiet,
        Command::Spawn(s) => s.common.quiet,
        Command::Validate(_) | Command::Kill(_) => false,
    }
}

fn init_tracing(quiet: bool) {
    let default = if quiet {
        "hyerix_natsfixture=error"
    } else {
        "hyerix_natsfixture=info,warn"
    };
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_env("HYERIX_NATSFIXTURE_LOG")
                .unwrap_or_else(|_| EnvFilter::new(default)),
        )
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .try_init();
}

async fn dispatch(cli: Cli) -> anyhow::Result<i32> {
    match cli.command {
        Command::Validate(a) => run_validate(&a.manifest).await,
        Command::Run(a) => run_foreground(&a.common).await,
        Command::Exec(a) => run_exec(&a.common, &a.argv).await,
        Command::Spawn(a) => run_spawn(&a.common, &a.pid_file).await,
        Command::Kill(a) => run_kill(&a.pid_file).await,
    }
}

async fn run_validate(path: &Path) -> anyhow::Result<i32> {
    match manifest::load(path) {
        Ok(m) => {
            tracing::info!(
                version = m.version,
                streams = m.streams.len(),
                consumers = m.consumers.len(),
                kv = m.kv.len(),
                object_store = m.object_store.len(),
                "manifest valid"
            );
            println!("OK");
            Ok(exit_codes::OK)
        }
        Err(e) => {
            eprintln!("manifest validation failed: {e}");
            Ok(exit_codes::MANIFEST_INVALID)
        }
    }
}

fn server_options(common: &CommonRunArgs) -> ServerOptions {
    let strategy = if common.prefer_system_nats_server {
        Resolution::SystemFirst
    } else {
        Resolution::BundledFirst
    };
    ServerOptions {
        bundled_strategy: strategy,
        override_binary: common.nats_server.clone(),
        host_override: common.host.clone(),
        port_override: common.port,
        keep_storage: common.keep_storage,
    }
}

async fn load_manifest_or_exit(path: &Path) -> anyhow::Result<(manifest::Manifest, PathBuf)> {
    let manifest = match manifest::load(path) {
        Ok(m) => m,
        Err(e @ ManifestError::UnsupportedVersion { .. }) => {
            eprintln!("{e}");
            std::process::exit(exit_codes::MANIFEST_INVALID);
        }
        Err(e) => {
            eprintln!("manifest validation failed: {e}");
            std::process::exit(exit_codes::MANIFEST_INVALID);
        }
    };
    let dir = path
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."));
    Ok((manifest, dir))
}

async fn boot_and_apply(
    common: &CommonRunArgs,
) -> anyhow::Result<(server::RunningServer, async_nats::Client)> {
    let (manifest, manifest_dir) = load_manifest_or_exit(&common.manifest).await?;
    let opts = server_options(common);

    let running = match server::start(&manifest, &opts).await {
        Ok(r) => r,
        Err(e) => {
            eprintln!("nats-server failed to start: {e:#}");
            std::process::exit(exit_codes::SERVER_FAILED);
        }
    };

    let client = match connect_with_auth(&manifest, &running.url).await {
        Ok(c) => c,
        Err(e) => {
            eprintln!(
                "could not connect to fixture NATS at {}: {e:#}",
                running.url
            );
            let _ = running.shutdown().await;
            std::process::exit(exit_codes::SERVER_FAILED);
        }
    };

    if let Err(e) = manifest::apply::apply(&client, &manifest, &manifest_dir).await {
        eprintln!("failed applying manifest: {e:#}");
        let _ = running.shutdown().await;
        std::process::exit(exit_codes::APPLY_FAILED);
    }

    if let Some(p) = &common.url_file {
        if let Err(e) = url::write_url_file(p, &running.url).await {
            eprintln!("failed writing --url-file: {e:#}");
            let _ = running.shutdown().await;
            std::process::exit(exit_codes::SERVER_FAILED);
        }
    }

    if let Err(e) = url::emit_banner(&running.url) {
        eprintln!("failed emitting URL banner: {e:#}");
        let _ = running.shutdown().await;
        std::process::exit(exit_codes::SERVER_FAILED);
    }

    Ok((running, client))
}

async fn connect_with_auth(
    m: &manifest::Manifest,
    url: &str,
) -> anyhow::Result<async_nats::Client> {
    use async_nats::ConnectOptions;
    let opts = match m.auth.mode {
        manifest::AuthMode::None => ConnectOptions::new(),
        manifest::AuthMode::Token => ConnectOptions::new().token(m.auth.token.clone()),
        manifest::AuthMode::UserPassword => {
            let u =
                m.auth.users.first().ok_or_else(|| {
                    anyhow::anyhow!("auth.mode=user_password but no users defined")
                })?;
            ConnectOptions::with_user_and_password(u.user.clone(), u.password.clone())
        }
        manifest::AuthMode::Nkey | manifest::AuthMode::Jwt => {
            anyhow::bail!("auth.mode nkey/jwt not supported in v0.1");
        }
    };
    let client = opts.name("hyerix-natsfixture-apply").connect(url).await?;
    Ok(client)
}

async fn run_foreground(common: &CommonRunArgs) -> anyhow::Result<i32> {
    let (running, _client) = boot_and_apply(common).await?;
    wait_for_signal().await;
    running.shutdown().await?;
    Ok(exit_codes::OK)
}

async fn run_exec(common: &CommonRunArgs, argv: &[String]) -> anyhow::Result<i32> {
    let (running, _client) = boot_and_apply(common).await?;
    let host = running.host.clone();
    let port = running.port;
    let url = running.url.clone();
    let result = exec::run_child(argv, &host, port, &url).await;
    let code = match &result {
        Ok(o) => o.exit_code,
        Err(_) => 1,
    };
    running.shutdown().await?;
    result.map(|_| code).or_else(|e| {
        eprintln!("exec failure: {e:#}");
        Ok(1)
    })
}

async fn run_spawn(common: &CommonRunArgs, pid_file: &Path) -> anyhow::Result<i32> {
    let self_path = std::env::current_exe()?;
    let mut cmd = std::process::Command::new(self_path);
    cmd.arg("run").arg("--manifest").arg(&common.manifest);
    if let Some(p) = common.port {
        cmd.arg("--port").arg(p.to_string());
    }
    if let Some(h) = &common.host {
        cmd.arg("--host").arg(h);
    }
    if let Some(b) = &common.nats_server {
        cmd.arg("--nats-server").arg(b);
    }
    if common.prefer_system_nats_server {
        cmd.arg("--prefer-system-nats-server");
    }
    if let Some(uf) = &common.url_file {
        cmd.arg("--url-file").arg(uf);
    }
    if common.keep_storage {
        cmd.arg("--keep-storage");
    }
    if common.quiet {
        cmd.arg("--quiet");
    }

    let child = cmd
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .spawn()?;
    let pid = child.id();

    std::fs::write(pid_file, format!("{pid}\n"))?;
    println!("PID={pid}");
    Ok(exit_codes::OK)
}

async fn run_kill(pid_file: &Path) -> anyhow::Result<i32> {
    let raw = std::fs::read_to_string(pid_file)?;
    let pid: i32 = raw.trim().parse()?;
    #[cfg(unix)]
    {
        extern "C" {
            fn kill(pid: i32, sig: i32) -> i32;
        }
        unsafe {
            kill(pid, 15);
        }
        let _ = std::fs::remove_file(pid_file);
        Ok(exit_codes::OK)
    }
    #[cfg(not(unix))]
    {
        let _ = pid;
        eprintln!(
            "kill subcommand not implemented on this platform; terminate the process directly"
        );
        Ok(exit_codes::SERVER_FAILED)
    }
}

async fn wait_for_signal() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};
        let mut term = signal(SignalKind::terminate()).expect("install SIGTERM handler");
        let mut int = signal(SignalKind::interrupt()).expect("install SIGINT handler");
        tokio::select! {
            _ = term.recv() => {},
            _ = int.recv() => {},
        }
    }
    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
}
