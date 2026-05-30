"""
conftest.py — pytest fixture that boots hyerix-natsfixture once per session.

Tests access the NATS URL through `nats_url` (function-scoped) or `session_nats_url`.

Alternative: just wrap your pytest invocation with `hyerix-natsfixture exec`:

    hyerix-natsfixture exec --manifest fixture.yaml -- pytest tests/

That's simpler if you don't need pytest fixtures — pick whichever fits.
"""

import os
import signal
import subprocess
import time
from pathlib import Path

import pytest


@pytest.fixture(scope="session")
def session_nats_url(tmp_path_factory):
    """Boot the fixture once per pytest session."""
    workdir = tmp_path_factory.mktemp("hyerix-natsfixture")
    url_file = workdir / "nats-url"
    pid_file = workdir / "nats-pid"

    proc = subprocess.Popen(
        [
            "hyerix-natsfixture",
            "spawn",
            "--manifest",
            "fixture.yaml",
            "--url-file",
            str(url_file),
            "--pid-file",
            str(pid_file),
        ],
    )

    # Wait for the URL file (fixture writes it once NATS_FIXTURE_READY is reached).
    deadline = time.monotonic() + 5
    while not url_file.exists():
        if time.monotonic() > deadline:
            proc.kill()
            raise RuntimeError("hyerix-natsfixture did not start within 5s")
        time.sleep(0.05)

    url = url_file.read_text().strip()
    yield url

    # Teardown — kill via the pid file the fixture wrote.
    subprocess.run(
        ["hyerix-natsfixture", "kill", "--pid-file", str(pid_file)],
        check=False,
    )
    proc.wait(timeout=5)


@pytest.fixture
def nats_url(session_nats_url):
    """Function-scoped alias that also injects NATS_URL into os.environ."""
    os.environ["NATS_URL"] = session_nats_url
    yield session_nats_url
    os.environ.pop("NATS_URL", None)
