use std::process::Stdio;

use anyhow::{Context, Result};
use tokio::process::Command;

pub struct ExecOutcome {
    pub exit_code: i32,
}

pub async fn run_child(argv: &[String], host: &str, port: u16, url: &str) -> Result<ExecOutcome> {
    if argv.is_empty() {
        anyhow::bail!("exec mode requires a command after `--`");
    }
    let (program, rest) = argv.split_first().unwrap();
    let mut cmd = Command::new(program);
    cmd.args(rest);
    cmd.env("NATS_URL", url);
    cmd.env("NATS_HOST", host);
    cmd.env("NATS_PORT", port.to_string());
    cmd.stdin(Stdio::inherit());
    cmd.stdout(Stdio::inherit());
    cmd.stderr(Stdio::inherit());

    let mut child = cmd
        .spawn()
        .with_context(|| format!("spawning wrapped command '{program}'"))?;
    let status = child
        .wait()
        .await
        .with_context(|| format!("waiting on wrapped command '{program}'"))?;
    let code = status.code().unwrap_or(1);
    Ok(ExecOutcome { exit_code: code })
}
