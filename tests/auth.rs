use std::path::PathBuf;
use std::time::Duration;

use hyerix_natsfixture::manifest;
use hyerix_natsfixture::server::{self, bundled::Resolution, ServerOptions};

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

fn opts() -> ServerOptions {
    ServerOptions {
        bundled_strategy: Resolution::SystemFirst,
        override_binary: None,
        host_override: None,
        port_override: None,
        keep_storage: false,
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn auth_none_anonymous_connects() {
    if !nats_server_available() {
        eprintln!("skipping: nats-server binary not on PATH");
        return;
    }
    let m = manifest::load(&fixture("minimal.yaml")).expect("load");
    let running = server::start(&m, &opts()).await.expect("start");
    let _client = async_nats::connect(&running.url).await.expect("connect");
    tokio::time::sleep(Duration::from_millis(50)).await;
    running.shutdown().await.expect("shutdown");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn auth_token_requires_token() {
    if !nats_server_available() {
        eprintln!("skipping: nats-server binary not on PATH");
        return;
    }
    let m = manifest::load(&fixture("auth_token.yaml")).expect("load");
    let running = server::start(&m, &opts()).await.expect("start");

    let anon = async_nats::connect(&running.url).await;
    assert!(anon.is_err(), "anonymous connect should be refused");

    let good = async_nats::ConnectOptions::new()
        .token("fixture-secret".into())
        .connect(&running.url)
        .await;
    assert!(good.is_ok(), "token connect should succeed: {good:?}");

    running.shutdown().await.expect("shutdown");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn auth_user_password_requires_credentials() {
    if !nats_server_available() {
        eprintln!("skipping: nats-server binary not on PATH");
        return;
    }
    let m = manifest::load(&fixture("auth_userpass.yaml")).expect("load");
    let running = server::start(&m, &opts()).await.expect("start");

    let anon = async_nats::connect(&running.url).await;
    assert!(anon.is_err(), "anonymous connect should be refused");

    let good =
        async_nats::ConnectOptions::with_user_and_password("testuser".into(), "testpass".into())
            .connect(&running.url)
            .await;
    assert!(good.is_ok(), "user/pass connect should succeed: {good:?}");

    running.shutdown().await.expect("shutdown");
}
