use std::path::Path;

use anyhow::{anyhow, Context, Result};
use async_nats::jetstream;
use async_nats::HeaderMap;

use super::{
    AckPolicy, Consumer, DeliverPolicy, KvBucket, Manifest, ObjectBucket, Retention, SeedMessage,
    StorageBackend, Stream,
};

pub async fn apply(
    client: &async_nats::Client,
    manifest: &Manifest,
    manifest_dir: &Path,
) -> Result<()> {
    let js = jetstream::new(client.clone());

    for s in &manifest.streams {
        create_stream(&js, s).await?;
        for seed in &s.seed {
            publish_seed(&js, &s.name, seed, manifest_dir).await?;
        }
    }

    for c in &manifest.consumers {
        create_consumer(&js, c).await?;
    }

    for b in &manifest.kv {
        create_kv(&js, b).await?;
    }

    for b in &manifest.object_store {
        create_object_store(&js, b).await?;
    }

    Ok(())
}

async fn create_stream(js: &jetstream::Context, s: &Stream) -> Result<()> {
    let cfg = jetstream::stream::Config {
        name: s.name.clone(),
        subjects: s.subjects.clone(),
        retention: match s.retention {
            Retention::Limits => jetstream::stream::RetentionPolicy::Limits,
            Retention::Interest => jetstream::stream::RetentionPolicy::Interest,
            Retention::Workqueue => jetstream::stream::RetentionPolicy::WorkQueue,
        },
        storage: match s.storage {
            StorageBackend::File => jetstream::stream::StorageType::File,
            StorageBackend::Memory => jetstream::stream::StorageType::Memory,
        },
        num_replicas: 1,
        max_messages: s.max_msgs,
        max_bytes: s.max_bytes,
        max_age: s.max_age.unwrap_or_default(),
        discard: match s.discard {
            super::Discard::Old => jetstream::stream::DiscardPolicy::Old,
            super::Discard::New => jetstream::stream::DiscardPolicy::New,
        },
        duplicate_window: s.duplicate_window.unwrap_or_default(),
        ..Default::default()
    };
    js.create_stream(cfg)
        .await
        .with_context(|| format!("creating stream '{}'", s.name))?;
    Ok(())
}

async fn publish_seed(
    js: &jetstream::Context,
    stream: &str,
    seed: &SeedMessage,
    manifest_dir: &Path,
) -> Result<()> {
    let payload: Vec<u8> = match (&seed.payload, &seed.payload_file) {
        (Some(p), None) => p.as_bytes().to_vec(),
        (None, Some(path)) => {
            let resolved = if path.is_absolute() {
                path.clone()
            } else {
                manifest_dir.join(path)
            };
            tokio::fs::read(&resolved)
                .await
                .with_context(|| format!("reading seed payload_file '{}'", resolved.display()))?
        }
        (None, None) => Vec::new(),
        (Some(_), Some(_)) => {
            return Err(anyhow!(
                "seed for stream '{}' subject '{}' declares both payload and payload_file",
                stream,
                seed.subject
            ));
        }
    };

    let mut headers = HeaderMap::new();
    for (k, v) in &seed.headers {
        headers.insert(k.as_str(), v.as_str());
    }

    let ack = if seed.headers.is_empty() {
        js.publish(seed.subject.clone(), payload.into()).await
    } else {
        js.publish_with_headers(seed.subject.clone(), headers, payload.into())
            .await
    };

    ack.with_context(|| format!("publishing seed to '{}'", seed.subject))?
        .await
        .with_context(|| format!("ack for seed publish on '{}'", seed.subject))?;

    Ok(())
}

async fn create_consumer(js: &jetstream::Context, c: &Consumer) -> Result<()> {
    let stream = js
        .get_stream(&c.stream)
        .await
        .with_context(|| format!("looking up stream '{}' for consumer '{}'", c.stream, c.name))?;

    let cfg = jetstream::consumer::pull::Config {
        durable_name: Some(c.name.clone()),
        name: Some(c.name.clone()),
        deliver_policy: match c.deliver_policy {
            DeliverPolicy::All => jetstream::consumer::DeliverPolicy::All,
            DeliverPolicy::Last => jetstream::consumer::DeliverPolicy::Last,
            DeliverPolicy::New => jetstream::consumer::DeliverPolicy::New,
            DeliverPolicy::ByStartSequence => {
                jetstream::consumer::DeliverPolicy::ByStartSequence { start_sequence: 1 }
            }
            DeliverPolicy::ByStartTime => jetstream::consumer::DeliverPolicy::New,
        },
        ack_policy: match c.ack_policy {
            AckPolicy::None => jetstream::consumer::AckPolicy::None,
            AckPolicy::All => jetstream::consumer::AckPolicy::All,
            AckPolicy::Explicit => jetstream::consumer::AckPolicy::Explicit,
        },
        max_deliver: c.max_deliver,
        ack_wait: c.ack_wait.unwrap_or_default(),
        filter_subject: c.filter_subject.clone().unwrap_or_default(),
        ..Default::default()
    };

    stream
        .create_consumer(cfg)
        .await
        .with_context(|| format!("creating consumer '{}' on stream '{}'", c.name, c.stream))?;
    Ok(())
}

async fn create_kv(js: &jetstream::Context, b: &KvBucket) -> Result<()> {
    let cfg = jetstream::kv::Config {
        bucket: b.bucket.clone(),
        history: b.history as i64,
        max_age: b.ttl.unwrap_or_default(),
        storage: match b.storage {
            StorageBackend::File => jetstream::stream::StorageType::File,
            StorageBackend::Memory => jetstream::stream::StorageType::Memory,
        },
        ..Default::default()
    };
    let store = js
        .create_key_value(cfg)
        .await
        .with_context(|| format!("creating KV bucket '{}'", b.bucket))?;
    for (k, v) in &b.seed {
        store
            .put(k, v.as_bytes().to_vec().into())
            .await
            .with_context(|| format!("seeding KV '{}' key '{}'", b.bucket, k))?;
    }
    Ok(())
}

async fn create_object_store(js: &jetstream::Context, b: &ObjectBucket) -> Result<()> {
    let cfg = jetstream::object_store::Config {
        bucket: b.bucket.clone(),
        max_age: b.ttl.unwrap_or_default(),
        storage: match b.storage {
            StorageBackend::File => jetstream::stream::StorageType::File,
            StorageBackend::Memory => jetstream::stream::StorageType::Memory,
        },
        ..Default::default()
    };
    js.create_object_store(cfg)
        .await
        .with_context(|| format!("creating object store bucket '{}'", b.bucket))?;
    Ok(())
}
