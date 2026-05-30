pub mod apply;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const SUPPORTED_SCHEMA_VERSION: &str = "1";

#[derive(Debug, Error)]
pub enum ManifestError {
    #[error("failed to read manifest file '{path}': {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to parse manifest YAML '{path}': {source}")]
    Parse {
        path: PathBuf,
        #[source]
        source: serde_yaml::Error,
    },
    #[error(
        "unsupported manifest version '{found}'. This fixture only understands version '{}'.",
        SUPPORTED_SCHEMA_VERSION
    )]
    UnsupportedVersion { found: String },
    #[error("manifest validation failed: {0}")]
    Invalid(String),
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    #[serde(default = "default_version")]
    pub version: String,

    #[serde(default)]
    pub listen: Listen,

    #[serde(default)]
    pub storage: Storage,

    #[serde(default)]
    pub streams: Vec<Stream>,

    #[serde(default)]
    pub consumers: Vec<Consumer>,

    #[serde(default)]
    pub kv: Vec<KvBucket>,

    #[serde(default)]
    pub object_store: Vec<ObjectBucket>,

    #[serde(default)]
    pub auth: Auth,

    #[serde(default)]
    pub log: Log,
}

fn default_version() -> String {
    SUPPORTED_SCHEMA_VERSION.to_string()
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Listen {
    #[serde(default = "default_host")]
    pub host: String,
    #[serde(default)]
    pub port: u16,
}

impl Default for Listen {
    fn default() -> Self {
        Self {
            host: default_host(),
            port: 0,
        }
    }
}

fn default_host() -> String {
    "127.0.0.1".to_string()
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(deny_unknown_fields)]
pub struct Storage {
    #[serde(default)]
    pub dir: String,
    #[serde(default = "default_js_max_memory")]
    pub jetstream_max_memory: String,
    #[serde(default = "default_js_max_file")]
    pub jetstream_max_file: String,
}

fn default_js_max_memory() -> String {
    "1G".to_string()
}
fn default_js_max_file() -> String {
    "10G".to_string()
}

#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Retention {
    #[default]
    Limits,
    Interest,
    Workqueue,
}

#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum StorageBackend {
    #[default]
    File,
    Memory,
}

#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Discard {
    #[default]
    Old,
    New,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Stream {
    pub name: String,
    #[serde(default)]
    pub subjects: Vec<String>,
    #[serde(default)]
    pub retention: Retention,
    #[serde(default)]
    pub storage: StorageBackend,
    #[serde(default = "default_replicas")]
    pub replicas: usize,
    #[serde(default = "default_max_msgs")]
    pub max_msgs: i64,
    #[serde(default = "default_max_bytes")]
    pub max_bytes: i64,
    #[serde(default, with = "humantime_serde")]
    pub max_age: Option<Duration>,
    #[serde(default)]
    pub discard: Discard,
    #[serde(default = "default_duplicate_window", with = "humantime_serde")]
    pub duplicate_window: Option<Duration>,
    #[serde(default)]
    pub seed: Vec<SeedMessage>,
}

fn default_replicas() -> usize {
    1
}
fn default_max_msgs() -> i64 {
    -1
}
fn default_max_bytes() -> i64 {
    -1
}
fn default_duplicate_window() -> Option<Duration> {
    Some(Duration::from_secs(120))
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SeedMessage {
    pub subject: String,
    #[serde(default)]
    pub payload: Option<String>,
    #[serde(default)]
    pub payload_file: Option<PathBuf>,
    #[serde(default)]
    pub headers: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DeliverPolicy {
    #[default]
    All,
    Last,
    New,
    ByStartSequence,
    ByStartTime,
}

#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AckPolicy {
    None,
    All,
    #[default]
    Explicit,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Consumer {
    pub stream: String,
    pub name: String,
    #[serde(default)]
    pub deliver_policy: DeliverPolicy,
    #[serde(default)]
    pub ack_policy: AckPolicy,
    #[serde(default = "default_max_deliver")]
    pub max_deliver: i64,
    #[serde(default = "default_ack_wait", with = "humantime_serde")]
    pub ack_wait: Option<Duration>,
    #[serde(default)]
    pub filter_subject: Option<String>,
}

fn default_max_deliver() -> i64 {
    -1
}
fn default_ack_wait() -> Option<Duration> {
    Some(Duration::from_secs(30))
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct KvBucket {
    pub bucket: String,
    #[serde(default = "default_history")]
    pub history: u8,
    #[serde(default, with = "humantime_serde")]
    pub ttl: Option<Duration>,
    #[serde(default)]
    pub storage: StorageBackend,
    #[serde(default)]
    pub seed: BTreeMap<String, String>,
}

fn default_history() -> u8 {
    1
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ObjectBucket {
    pub bucket: String,
    #[serde(default)]
    pub storage: StorageBackend,
    #[serde(default, with = "humantime_serde")]
    pub ttl: Option<Duration>,
}

#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AuthMode {
    #[default]
    None,
    Token,
    UserPassword,
    Nkey,
    Jwt,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(deny_unknown_fields)]
pub struct Auth {
    #[serde(default)]
    pub mode: AuthMode,
    #[serde(default)]
    pub token: String,
    #[serde(default)]
    pub users: Vec<UserCredential>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct UserCredential {
    pub user: String,
    pub password: String,
    #[serde(default)]
    pub permissions: Option<UserPermissions>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(deny_unknown_fields)]
pub struct UserPermissions {
    #[serde(default)]
    pub publish: Vec<String>,
    #[serde(default)]
    pub subscribe: Vec<String>,
}

#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Off,
    Error,
    #[default]
    Warn,
    Info,
    Debug,
    Trace,
}

#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum LogFormat {
    #[default]
    Text,
    Json,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(deny_unknown_fields)]
pub struct Log {
    #[serde(default)]
    pub level: LogLevel,
    #[serde(default)]
    pub format: LogFormat,
    #[serde(default = "default_log_destination")]
    pub destination: String,
}

fn default_log_destination() -> String {
    "stderr".to_string()
}

pub fn load(path: &Path) -> Result<Manifest, ManifestError> {
    let raw = std::fs::read_to_string(path).map_err(|source| ManifestError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    parse_str(&raw, path)
}

pub fn parse_str(raw: &str, path_for_errors: &Path) -> Result<Manifest, ManifestError> {
    let manifest: Manifest = serde_yaml::from_str(raw).map_err(|source| ManifestError::Parse {
        path: path_for_errors.to_path_buf(),
        source,
    })?;
    validate(&manifest)?;
    Ok(manifest)
}

pub fn validate(m: &Manifest) -> Result<(), ManifestError> {
    if m.version != SUPPORTED_SCHEMA_VERSION {
        return Err(ManifestError::UnsupportedVersion {
            found: m.version.clone(),
        });
    }

    if m.listen.host.trim().is_empty() {
        return Err(ManifestError::Invalid(
            "listen.host must not be empty".into(),
        ));
    }

    let mut stream_names = std::collections::HashSet::new();
    for s in &m.streams {
        if s.name.trim().is_empty() {
            return Err(ManifestError::Invalid(
                "stream.name must not be empty".into(),
            ));
        }
        if !stream_names.insert(s.name.clone()) {
            return Err(ManifestError::Invalid(format!(
                "duplicate stream name '{}'",
                s.name
            )));
        }
        if s.replicas != 1 {
            return Err(ManifestError::Invalid(format!(
                "stream '{}' specifies replicas={}; this fixture is single-node, only replicas=1 is supported in v0.1",
                s.name, s.replicas
            )));
        }
        if s.subjects.is_empty() {
            return Err(ManifestError::Invalid(format!(
                "stream '{}' must declare at least one subject",
                s.name
            )));
        }
        for seed in &s.seed {
            if seed.payload.is_some() && seed.payload_file.is_some() {
                return Err(ManifestError::Invalid(format!(
                    "stream '{}' seed for subject '{}' has both `payload` and `payload_file` — choose one",
                    s.name, seed.subject
                )));
            }
        }
    }

    let mut consumer_keys = std::collections::HashSet::new();
    for c in &m.consumers {
        if c.name.trim().is_empty() {
            return Err(ManifestError::Invalid(
                "consumer.name must not be empty".into(),
            ));
        }
        if c.stream.trim().is_empty() {
            return Err(ManifestError::Invalid(format!(
                "consumer '{}' must reference a stream",
                c.name
            )));
        }
        if !stream_names.contains(&c.stream) {
            return Err(ManifestError::Invalid(format!(
                "consumer '{}' references stream '{}' that is not defined in this manifest",
                c.name, c.stream
            )));
        }
        let key = (c.stream.clone(), c.name.clone());
        if !consumer_keys.insert(key) {
            return Err(ManifestError::Invalid(format!(
                "duplicate consumer '{}' on stream '{}'",
                c.name, c.stream
            )));
        }
    }

    let mut kv_names = std::collections::HashSet::new();
    for b in &m.kv {
        if b.bucket.trim().is_empty() {
            return Err(ManifestError::Invalid("kv.bucket must not be empty".into()));
        }
        if !kv_names.insert(b.bucket.clone()) {
            return Err(ManifestError::Invalid(format!(
                "duplicate KV bucket '{}'",
                b.bucket
            )));
        }
    }

    let mut obj_names = std::collections::HashSet::new();
    for b in &m.object_store {
        if b.bucket.trim().is_empty() {
            return Err(ManifestError::Invalid(
                "object_store.bucket must not be empty".into(),
            ));
        }
        if !obj_names.insert(b.bucket.clone()) {
            return Err(ManifestError::Invalid(format!(
                "duplicate object_store bucket '{}'",
                b.bucket
            )));
        }
    }

    match m.auth.mode {
        AuthMode::Token => {
            if m.auth.token.is_empty() {
                return Err(ManifestError::Invalid(
                    "auth.mode=token requires a non-empty auth.token".into(),
                ));
            }
        }
        AuthMode::UserPassword => {
            if m.auth.users.is_empty() {
                return Err(ManifestError::Invalid(
                    "auth.mode=user_password requires at least one entry in auth.users".into(),
                ));
            }
            for u in &m.auth.users {
                if u.user.is_empty() || u.password.is_empty() {
                    return Err(ManifestError::Invalid(
                        "auth.users entries must have non-empty user and password".into(),
                    ));
                }
            }
        }
        AuthMode::Nkey | AuthMode::Jwt => {
            return Err(ManifestError::Invalid(format!(
                "auth.mode={:?} is reserved in the schema but not implemented in v0.1",
                m.auth.mode
            )));
        }
        AuthMode::None => {}
    }

    Ok(())
}
