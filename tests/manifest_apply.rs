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

#[test]
fn full_manifest_parses_cleanly() {
    let m = manifest::load(&fixture("full.yaml")).expect("load");
    assert_eq!(m.version, "1");
    assert_eq!(m.streams.len(), 1);
    assert_eq!(m.streams[0].name, "ORDERS");
    assert_eq!(m.streams[0].seed.len(), 1);
    assert_eq!(m.consumers.len(), 1);
    assert_eq!(m.consumers[0].stream, "ORDERS");
    assert_eq!(m.kv.len(), 1);
    assert_eq!(m.kv[0].bucket, "feature-flags");
    assert_eq!(m.object_store.len(), 1);
}

#[test]
fn minimal_manifest_defaults() {
    let m = manifest::load(&fixture("minimal.yaml")).expect("load");
    assert_eq!(m.version, "1");
    assert_eq!(m.listen.host, "127.0.0.1");
    assert_eq!(m.listen.port, 0);
    assert_eq!(m.streams.len(), 1);
    assert_eq!(m.streams[0].subjects, vec!["orders.>"]);
}

#[test]
fn unknown_field_rejected() {
    let yaml = "version: \"1\"\nnope: 1\n";
    let err = manifest::parse_str(yaml, std::path::Path::new("inline")).unwrap_err();
    let s = format!("{err}");
    assert!(s.contains("nope") || s.contains("unknown"), "got: {s}");
}

#[test]
fn consumer_referencing_missing_stream_rejected() {
    let yaml = r#"
version: "1"
consumers:
  - stream: NOPE
    name: bogus
"#;
    let err = manifest::parse_str(yaml, std::path::Path::new("inline")).unwrap_err();
    let s = format!("{err}");
    assert!(s.contains("NOPE") || s.contains("not defined"), "got: {s}");
}

#[test]
fn seed_with_both_payload_and_file_rejected() {
    let yaml = r#"
version: "1"
streams:
  - name: S
    subjects: ["s.>"]
    seed:
      - subject: s.x
        payload: "hi"
        payload_file: "./missing.txt"
"#;
    let err = manifest::parse_str(yaml, std::path::Path::new("inline")).unwrap_err();
    let s = format!("{err}");
    assert!(s.contains("payload"), "got: {s}");
}

#[test]
fn replicas_other_than_one_rejected() {
    let yaml = r#"
version: "1"
streams:
  - name: S
    subjects: ["s.>"]
    replicas: 3
"#;
    let err = manifest::parse_str(yaml, std::path::Path::new("inline")).unwrap_err();
    let s = format!("{err}");
    assert!(
        s.contains("replicas") || s.contains("single-node"),
        "got: {s}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn applying_full_manifest_creates_streams_kv_objects() {
    if !nats_server_available() {
        eprintln!("skipping: nats-server binary not on PATH");
        return;
    }

    let m = manifest::load(&fixture("full.yaml")).expect("load");
    let opts = ServerOptions {
        bundled_strategy: Resolution::SystemFirst,
        override_binary: None,
        host_override: None,
        port_override: None,
        keep_storage: false,
    };
    let running = server::start(&m, &opts).await.expect("start");
    let client = async_nats::connect(&running.url).await.expect("connect");
    let manifest_path = fixture("full.yaml");
    let manifest_dir = manifest_path.parent().unwrap();
    manifest::apply::apply(&client, &m, manifest_dir)
        .await
        .expect("apply");

    let js = async_nats::jetstream::new(client.clone());
    let stream = js.get_stream("ORDERS").await.expect("get stream ORDERS");
    let info = stream.cached_info();
    assert_eq!(info.config.name, "ORDERS");

    let kv = js.get_key_value("feature-flags").await.expect("kv bucket");
    let v = kv
        .get("payments_v2")
        .await
        .expect("kv get")
        .expect("present");
    assert_eq!(&v[..], b"true");

    let obj = js
        .get_object_store("invoices")
        .await
        .expect("object bucket");
    drop(obj);

    drop(client);
    tokio::time::sleep(Duration::from_millis(50)).await;
    running.shutdown().await.expect("shutdown");
}
