use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_hyerix-natsfixture")
}

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

fn nats_server_available() -> bool {
    if which::which("nats-server").is_ok() {
        return true;
    }
    if std::env::var("CI").is_ok() {
        panic!(
            "nats-server required in CI but not on PATH — install step missing from .github/workflows/ci.yml"
        );
    }
    false
}

#[test]
fn version_flag_prints_crate_and_bundled_version() {
    let out = Command::new(bin())
        .arg("--version")
        .output()
        .expect("spawn");
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("0.1.0"),
        "expected crate version in --version output: {stdout}"
    );
    assert!(
        stdout.contains("bundled nats-server"),
        "expected bundled nats-server marker in --version output: {stdout}"
    );
}

#[test]
fn help_lists_all_five_subcommands() {
    let out = Command::new(bin()).arg("--help").output().expect("spawn");
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    for sub in ["run", "validate", "exec", "spawn", "kill"] {
        assert!(
            stdout.contains(sub),
            "help missing subcommand '{sub}': {stdout}"
        );
    }
}

#[test]
fn validate_ok_exits_zero() {
    let out = Command::new(bin())
        .arg("validate")
        .arg("--manifest")
        .arg(fixture("minimal.yaml"))
        .output()
        .expect("spawn");
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn validate_bad_version_exits_one() {
    let out = Command::new(bin())
        .arg("validate")
        .arg("--manifest")
        .arg(fixture("invalid_version.yaml"))
        .output()
        .expect("spawn");
    assert_eq!(out.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("version") || stderr.contains("unsupported"),
        "stderr missing version error: {stderr}"
    );
}

#[test]
fn validate_dup_stream_exits_one() {
    let out = Command::new(bin())
        .arg("validate")
        .arg("--manifest")
        .arg(fixture("invalid_dup_stream.yaml"))
        .output()
        .expect("spawn");
    assert_eq!(out.status.code(), Some(1));
}

#[test]
fn run_minimal_emits_url_banner_and_ready_sentinel() {
    if !nats_server_available() {
        eprintln!("skipping: nats-server binary not on PATH (no bundled binary in dev tree)");
        return;
    }

    let mut child = Command::new(bin())
        .arg("run")
        .arg("--prefer-system-nats-server")
        .arg("--manifest")
        .arg(fixture("minimal.yaml"))
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn");

    let stdout = child.stdout.take().expect("stdout pipe");
    let mut reader = BufReader::new(stdout);
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut got_url = None;
    let mut got_ready = false;
    while Instant::now() < deadline {
        let mut line = String::new();
        let read = reader.read_line(&mut line).unwrap_or(0);
        if read == 0 {
            std::thread::sleep(Duration::from_millis(50));
            continue;
        }
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("NATS_URL=") {
            got_url = Some(rest.to_string());
        }
        if trimmed == "NATS_FIXTURE_READY" {
            got_ready = true;
        }
        if got_url.is_some() && got_ready {
            break;
        }
    }

    let _ = child.kill();
    let _ = child.wait();

    let url = got_url.expect("never saw NATS_URL= banner");
    assert!(
        url.starts_with("nats://127.0.0.1:"),
        "unexpected URL banner: {url}"
    );
    assert!(got_ready, "never saw NATS_FIXTURE_READY sentinel");
}

#[test]
fn exec_mode_runs_child_with_nats_env_vars() {
    if !nats_server_available() {
        eprintln!("skipping: nats-server binary not on PATH");
        return;
    }

    let (script_program, script_args): (&str, Vec<String>) = if cfg!(windows) {
        (
            "cmd",
            vec![
                "/C".into(),
                "echo URL=%NATS_URL% HOST=%NATS_HOST% PORT=%NATS_PORT%".into(),
            ],
        )
    } else {
        (
            "sh",
            vec![
                "-c".into(),
                "printf 'URL=%s HOST=%s PORT=%s\\n' \"$NATS_URL\" \"$NATS_HOST\" \"$NATS_PORT\""
                    .into(),
            ],
        )
    };

    let mut cmd = Command::new(bin());
    cmd.arg("exec")
        .arg("--prefer-system-nats-server")
        .arg("--manifest")
        .arg(fixture("minimal.yaml"))
        .arg("--")
        .arg(script_program);
    for a in script_args {
        cmd.arg(a);
    }

    let out = cmd.output().expect("spawn");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success(),
        "exec mode failed. stdout={stdout} stderr={stderr}"
    );
    let combined = format!("{stdout}\n{stderr}");
    assert!(
        combined.contains("URL=nats://127.0.0.1:"),
        "child did not see NATS_URL in env. combined={combined}"
    );
    assert!(
        combined.contains("HOST=127.0.0.1"),
        "child did not see NATS_HOST in env. combined={combined}"
    );
    assert!(
        combined.contains("PORT="),
        "child did not see NATS_PORT in env. combined={combined}"
    );
}

#[test]
fn exec_mode_propagates_child_exit_code() {
    if !nats_server_available() {
        eprintln!("skipping: nats-server binary not on PATH");
        return;
    }

    let (program, args): (&str, Vec<&str>) = if cfg!(windows) {
        ("cmd", vec!["/C", "exit 42"])
    } else {
        ("sh", vec!["-c", "exit 42"])
    };

    let mut cmd = Command::new(bin());
    cmd.arg("exec")
        .arg("--prefer-system-nats-server")
        .arg("--manifest")
        .arg(fixture("minimal.yaml"))
        .arg("--")
        .arg(program);
    for a in args {
        cmd.arg(a);
    }

    let out = cmd.output().expect("spawn");
    assert_eq!(
        out.status.code(),
        Some(42),
        "child's exit 42 should propagate; stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn url_file_is_written() {
    if !nats_server_available() {
        eprintln!("skipping: nats-server binary not on PATH");
        return;
    }

    let dir = tempfile::tempdir().expect("tempdir");
    let url_file = dir.path().join("nats.url");

    let (program, args): (&str, Vec<String>) = if cfg!(windows) {
        ("cmd", vec!["/C".into(), "ver".into()])
    } else {
        ("sh", vec!["-c".into(), "true".into()])
    };

    let mut cmd = Command::new(bin());
    cmd.arg("exec")
        .arg("--prefer-system-nats-server")
        .arg("--manifest")
        .arg(fixture("minimal.yaml"))
        .arg("--url-file")
        .arg(&url_file)
        .arg("--")
        .arg(program);
    for a in args {
        cmd.arg(a);
    }
    let out = cmd.output().expect("spawn");
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let contents = std::fs::read_to_string(&url_file).expect("read url file");
    assert!(
        contents.trim().starts_with("nats://127.0.0.1:"),
        "url file content: {contents:?}"
    );
}
