# hyerix-natsfixture

[![CI](https://github.com/hyerix/hyerix-natsfixture/actions/workflows/ci.yml/badge.svg)](https://github.com/hyerix/hyerix-natsfixture/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/hyerix/hyerix-natsfixture?color=00D4AA&label=release)](https://github.com/hyerix/hyerix-natsfixture/releases)
[![License: Apache 2.0](https://img.shields.io/badge/License-Apache_2.0-00D4AA.svg)](https://opensource.org/licenses/Apache-2.0)
[![Rust](https://img.shields.io/badge/rust-1.75%2B-orange.svg)](https://www.rust-lang.org)

**A polyglot CI fixture for NATS-dependent code.** Single Rust binary, bundled NATS server, declarative YAML manifest. Boots an ephemeral NATS + JetStream cluster pre-loaded with your test streams, consumers, KV buckets, and Object Store, hands you a connection URL, and cleans up on exit. No Docker. No language SDK. No `nats-server` install dance.

For dashboards, topology graphs, and Signal AI on top of the same NATS code path, see [Hyerix](https://hyerix.ai#download).

---

## What this is

A test fixture. Drop it into any language's CI suite, spin up a per-test (or per-suite) NATS cluster matching the shape your code expects, run your tests against it, exit cleanly.

## What this is not

- **Not for production.** This is an embedded NATS server explicitly for tests. The bundled `nats-server` exits with the fixture; do not point production traffic at it.
- **Not a long-running dev cluster.** For that, use `nats-server` directly, or [hyerix/hyerix-demo-cluster](https://github.com/hyerix/hyerix-demo-cluster).
- **Not a Hyerix licence check.** The fixture is unlicensed by design. Run it freely in any CI.

---

## Install

### Homebrew (macOS + Linux)

```sh
brew install hyerix/tap/hyerix-natsfixture
```

### Pre-built binary

Download the platform binary from the [releases page](https://github.com/hyerix/hyerix-natsfixture/releases), unpack, and add it to your `PATH`. The release tarball bundles the matching `nats-server` binary alongside; both end up in the same directory.

### From source

```sh
cargo install --git https://github.com/hyerix/hyerix-natsfixture
```

(Source builds do not bundle the `nats-server` binary. You will need `nats-server` on your `PATH` separately, or pass `--nats-server <path>`.)

---

## Quick start

Write a fixture manifest:

```yaml
# fixture.yaml
streams:
  - name: ORDERS
    subjects: ["orders.>"]
```

Run your tests against it:

```sh
hyerix-natsfixture exec --manifest fixture.yaml -- pnpm test
```

`exec` mode boots the fixture, sets `NATS_URL`, `NATS_HOST`, and `NATS_PORT` in the test command's environment, runs `pnpm test`, and tears the fixture down when the test command exits. The test command's exit code propagates.

For test frameworks that need decoupled lifecycle (global setup files, parallel test runners) the same fixture is reachable through `spawn` + `kill` — see [CLI](#cli) below.

---

## Why hyerix-natsfixture

- **Polyglot, single binary.** Existing fixtures are language-locked (Go-only, Java-only). This one is a flat executable. Use it from Jest, pytest, `cargo test`, `gradle test`, Go's `testing` package, or anything else that can shell out.
- **Bundled NATS, zero install dance.** The release tarball ships the matching `nats-server` binary inside it. No Docker pull, no separate `brew install nats-server`, no version drift between your CI and your prod.
- **Declarative manifest.** Tests need a stream with three subjects, two consumers, a KV bucket, and three seed messages? Write it in YAML. The fixture applies it deterministically every run.
- **<1s startup, deterministic shutdown.** Target startup to `NATS_FIXTURE_READY` is under a second. Shutdown drains in-flight requests with a hard 2s timeout, then removes the temp storage directory.

---

## The manifest

The minimum useful fixture is two lines (everything else has sensible defaults):

```yaml
streams:
  - name: ORDERS
    subjects: ["orders.>"]
```

A richer fixture exercising every section:

```yaml
version: "1"

listen:
  host: "127.0.0.1"
  port: 0                      # 0 = OS-assigned

storage:
  jetstream_max_memory: "1G"
  jetstream_max_file: "10G"

streams:
  - name: ORDERS
    subjects: ["orders.>"]
    retention: limits          # limits | interest | workqueue
    storage: file              # file | memory
    max_msgs: -1
    max_bytes: -1
    max_age: 0
    duplicate_window: 2m
    seed:
      - subject: orders.us.created
        payload: '{"id": "test-1", "amount": 100}'
        headers:
          Content-Type: application/json

consumers:
  - stream: ORDERS
    name: order-processor      # durable name
    deliver_policy: all        # all | last | new | by_start_sequence | by_start_time
    ack_policy: explicit       # none | all | explicit
    max_deliver: 5
    ack_wait: 30s
    filter_subject: "orders.us.>"

kv:
  - bucket: feature-flags
    history: 10
    seed:
      payments_v2: "true"
      shadow_writes: "false"

object_store:
  - bucket: invoices
    storage: file

auth:
  mode: none                   # none | token | user_password
  # token: "..."                 # used when mode: token
  # users:                       # used when mode: user_password
  #   - user: testuser
  #     password: testpass
  #     permissions:
  #       publish: ["orders.>"]
  #       subscribe: ["events.>"]

log:
  level: warn                  # off | error | warn | info | debug | trace
```

See [`examples/manifest.example.yaml`](./examples/manifest.example.yaml) for a copy-paste reference covering every field. Manifest validation is structural — required fields, no unknown fields, version pinned at `"1"`. Bad manifests fail at startup with a clear error, not silently mid-test.

---

## CLI

`hyerix-natsfixture` ships five subcommands plus three flags that apply to most of them.

| Command | What it does |
|---|---|
| `run` | Boot the fixture in the foreground. Blocks until `SIGINT` / `SIGTERM`. |
| `exec` | Boot the fixture, run a wrapped command with `NATS_URL` etc. in env, exit when the command exits. **Recommended for CI.** |
| `spawn` | Boot the fixture and detach. Prints the PID. Pair with `kill`. |
| `kill` | Stop a `spawn`-started fixture cleanly. |
| `validate` | Parse and validate a manifest without booting anything. |

Flags worth knowing:

| Flag | Effect |
|---|---|
| `--manifest <path>` | Path to the YAML manifest. Required for `run`, `exec`, `spawn`, `validate`. |
| `--port <n>` | Pin the listen port instead of OS-assigned. |
| `--url-file <path>` | Write the `NATS_URL` to a file (for test frameworks that need it on disk). |
| `--nats-server <path>` | Override the bundled binary. |
| `--prefer-system-nats-server` | Look on `PATH` first, fall back to bundled. |
| `--keep-storage` | Skip temp-dir cleanup on shutdown (for debugging a failed test). |
| `--quiet` | Suppress all fixture logs except errors and the URL banner. |

### Lifecycle modes

**Exec-around (recommended).** One command, no cleanup logic in your test scripts. Fixture lifecycle wraps the test command.

```sh
hyerix-natsfixture exec --manifest fixture.yaml -- pnpm test
```

**Foreground.** Useful for local development — boot the fixture once, run tests in another shell as many times as you want.

```sh
hyerix-natsfixture run --manifest fixture.yaml
# In another shell: NATS_URL=nats://127.0.0.1:<port> pnpm test
```

**Spawn + kill.** For test frameworks with global setup/teardown hooks that need the fixture lifecycle decoupled from a single command.

```sh
PID=$(hyerix-natsfixture spawn --manifest fixture.yaml --pid-file /tmp/hf.pid)
# ... your test runner does its thing ...
hyerix-natsfixture kill --pid-file /tmp/hf.pid
```

---

## Wiring into your test framework

Each example is ready to copy-paste:

| Language / framework | Example |
|---|---|
| Node.js + Jest | [`examples/jest.config.example.js`](./examples/jest.config.example.js) |
| Python + pytest | [`examples/pytest_conftest.py`](./examples/pytest_conftest.py) |
| Rust + `cargo test` | [`examples/code/cargo_integration.rs`](./examples/code/cargo_integration.rs) |
| GitHub Actions | [`examples/github-actions.yml`](./examples/github-actions.yml) |

Any language that can shell out and read environment variables (so: any language) works the same way. Use `exec` mode if you can; fall back to `spawn` + `kill` if your runner needs the lifecycle decoupled.

---

## Configuration

### Auth

The fixture defaults to anonymous (`auth.mode: none`) because the overwhelming majority of test suites do not exercise auth. Tests that *do* can opt into `token` or `user_password` modes via the manifest.

`nkey` and `jwt` modes are reserved in the schema for v0.2 — they parse but currently reject with a clear error.

### Connection URL discovery

Three mechanisms, all available:

- **Stdout banner** (always): the line `NATS_URL=nats://127.0.0.1:<port>` is printed on its own line at startup. Machine-parseable.
- **`--url-file <path>`**: writes the URL to a file for test frameworks that need it on disk.
- **`exec` mode auto-env**: `NATS_URL`, `NATS_HOST`, `NATS_PORT` are auto-populated in the child command's environment.

The `NATS_FIXTURE_READY` sentinel line is printed after the URL banner once the manifest has been applied. Wait on this in setup scripts to avoid race conditions.

### Exit codes

| Code | Meaning |
|---|---|
| 0 | Clean shutdown (foreground) OR child exited 0 (exec) |
| 1 | Manifest validation failure |
| 2 | Port already in use (when `--port` was specified explicitly) |
| 3 | NATS server failed to start |
| 4 | Manifest apply failure |
| _child's exit code_ | In `exec` mode, the wrapped command's exit code propagates |

### Compatibility

- **NATS server:** 2.10+ required for JetStream KV and Object Store. The release tarball bundles a known-good version (printed by `hyerix-natsfixture --version`).
- **Platforms:** macOS arm64 + x86_64, Linux x86_64 + arm64, Windows x86_64. Signed release binaries on every tag.
- **JetStream:** required. The fixture always enables JS on the embedded server; manifests without `streams`/`kv`/`object_store` blocks are fine — JS is just unused.

---

## Issues & contributions

Found a bug, hit a limitation, or want a manifest field we don't yet expose? Open an issue at [github.com/hyerix/hyerix-natsfixture/issues](https://github.com/hyerix/hyerix-natsfixture/issues).

## Security

Security issues: please **don't** open a public issue. See [SECURITY.md](./SECURITY.md) or email `security@hyerix.ai`.

---

## Built by Hyerix

[Hyerix](https://hyerix.ai) is the AI-native desktop GUI for NATS. `hyerix-natsfixture` is the CI sibling of the same NATS code path. Same NATS client work, different lifetime — long-running desktop for ops, short-lived fixture for tests.

License: Apache-2.0.

<sub>Hyerix <code>/ˈhaɪ.rɪks/</code> — rhymes with "high tricks". [@hyerixAI](https://x.com/hyerixAI)</sub>
