# Security Policy

## Reporting a vulnerability

Please **don't** open a public issue for security findings.

Email: **`security@hyerix.ai`**

Include:

- A description of the issue and steps to reproduce
- The affected version (`hyerix-natsfixture --version`)
- Any proof-of-concept commands or manifests
- Whether you've disclosed elsewhere yet

We acknowledge within 2 business days and aim to ship a fix within 30 days for confirmed issues.

## In scope

`hyerix-natsfixture` is a CI fixture that runs `nats-server` as a child process inside a temporary directory. Issues we treat as security-relevant:

- A manifest that causes the fixture to read or write outside its declared temp storage directory
- Anything that lets a manifest exceed declared bounds (port binding, file descriptors, child processes spawned)
- Memory-safety bugs (panics, crashes, resource exhaustion under malformed input)
- Authentication path bugs (`token` / `user_password` handling)
- Anything that lets the fixture leak credentials or paths into logs, error messages, or the stdout banner

## Out of scope

- Bugs in upstream `async-nats`, `nats-server`, `tokio` — please report those to the respective projects
- The behaviour of a test command that the fixture wraps in `exec` mode — the fixture has no opinion about what the wrapped command does
- Misconfigurations on the operator's side (running the fixture as root, sharing the temp directory across mutually-distrustful workloads, etc.)
- The bundled `nats-server` binary's own CVEs — those are upstream-scope, except when we've shipped a version with a known patched CVE without bumping the pin. In that case, please flag the version-pin lag and we'll ship a release with the patched version.

## Coordinated disclosure

If you're a researcher or vendor working under a coordinated-disclosure timeline, mention your preferred window in the initial email and we'll work with it.
