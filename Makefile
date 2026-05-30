.PHONY: dev build release test clippy fmt fmt-check check validate-minimal sync

dev:
	cargo run -- run --manifest tests/fixtures/minimal.yaml

build:
	cargo build

release:
	cargo build --release

test:
	cargo test --all

clippy:
	cargo clippy --all-targets -- -D warnings

fmt:
	cargo fmt

fmt-check:
	cargo fmt -- --check

check: fmt-check clippy test

validate-minimal:
	cargo run -- validate --manifest tests/fixtures/minimal.yaml

sync:
	@echo "hyerix-natsfixture has no shared source with hyerix-mcp yet; release-pipeline drift is the manual surface."
	@echo "Diff hint: diff -u ../hyerix-mcp/.github/workflows/release.yml .github/workflows/release.yml"
