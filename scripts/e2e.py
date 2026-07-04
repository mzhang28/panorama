#!/usr/bin/env python3
"""Panorama E2E Test Harness.

Runs Playwright against an isolated instance of Panorama server with a temporary
data directory on a random port. By default, assumes server and plugins are pre-built.
Use --build to run the build phase prior to starting the test server.
"""

import os
import shutil
import socket
import subprocess
import sys
import tempfile
import time
import urllib.error
import urllib.request
from pathlib import Path
from typing import Optional

import click


def find_free_port() -> int:
    """Find an available TCP port on localhost."""
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as s:
        s.bind(("127.0.0.1", 0))
        return s.getsockname()[1]


def wait_for_url(
    url: str, attempts: int = 60, interval: float = 0.5, desc: str = "server"
) -> bool:
    """Poll URL until it returns a successful response."""
    for _ in range(attempts):
        try:
            req = urllib.request.Request(url, headers={"User-Agent": "E2E-Harness"})
            with urllib.request.urlopen(req, timeout=1.0) as resp:
                if 200 <= resp.status < 400:
                    return True
        except (urllib.error.URLError, OSError, TimeoutError):
            pass
        time.sleep(interval)
    click.echo(f"  ✗ Timed out waiting for {desc} at {url}", err=True)
    return False


@click.command(
    context_settings=dict(
        ignore_unknown_options=True,
        allow_extra_args=True,
    ),
    help="Run E2E tests against an isolated Panorama server instance.",
)
@click.option(
    "--build",
    is_flag=True,
    default=False,
    help="Trigger a full build of frontend, WASM plugins, server, and panoapps before testing.",
)
@click.option(
    "--port",
    type=int,
    default=None,
    help="Port to run the server on (default: random free port).",
)
@click.pass_context
def main(ctx: click.Context, build: bool, port: Optional[int]) -> None:
    repo_root = Path(__file__).resolve().parent.parent
    os.chdir(repo_root)

    server_port = port if port is not None else find_free_port()
    playwright_args = ctx.args

    click.echo("=== Panorama E2E Harness ===")
    click.echo(f"  server   : 127.0.0.1:{server_port}")
    if playwright_args:
        click.echo(f"  playwright args: {' '.join(playwright_args)}")
    click.echo("")

    # Create temporary data directory
    temp_dir_obj = tempfile.TemporaryDirectory(prefix="panorama_e2e_")
    data_dir = Path(temp_dir_obj.name)
    plugins_dir = data_dir / "plugins"
    plugins_dir.mkdir(parents=True, exist_ok=True)

    server_process: Optional[subprocess.Popen] = None

    try:
        if build:
            click.echo("--- Building frontend (dev mode — workspace imports) ---")
            subprocess.run(
                ["bun", "install", "--silent"],
                cwd=repo_root / "frontend",
                check=True,
            )
            subprocess.run(
                ["bun", "x", "vite", "build", "--mode", "development"],
                cwd=repo_root / "frontend",
                check=True,
            )

            click.echo("\n--- Building WASM plugins ---")
            subprocess.run(["bash", "scripts/build-wasm.sh"], check=True)

            click.echo("\n--- Building server (embeds frontend via rust-embed) ---")
            subprocess.run(
                ["cargo", "build", "--release", "-p", "panorama-server"],
                check=True,
            )

            click.echo("\n--- Building plugin UIs + packaging .panoapp files ---")
            subprocess.run(["bash", "scripts/build-panoapp.sh"], check=True)
            click.echo("")

        # Copy .panoapp files into isolated data dir
        panoapp_dir = repo_root / "dist" / "panoapp"
        panoapp_files = (
            list(panoapp_dir.glob("*.panoapp")) if panoapp_dir.exists() else []
        )

        if not panoapp_files:
            click.echo(
                "  ✗ No .panoapp files found in dist/panoapp/ — build first (e.g. `just build`) or pass --build",
                err=True,
            )
            sys.exit(1)

        for src in panoapp_files:
            shutil.copy(src, plugins_dir / src.name)

        click.echo(f"  Copied {len(panoapp_files)} .panoapp files")
        click.echo("")

        # Check server binary exists
        server_bin = repo_root / "target" / "release" / "panorama-server"
        if not server_bin.exists():
            click.echo(
                f"  ✗ Server binary not found at {server_bin} — build first (e.g. `just build`) or pass --build",
                err=True,
            )
            sys.exit(1)

        # Start server
        click.echo("--- Starting server ---")
        env = os.environ.copy()
        env["PANORAMA_DATA_DIR"] = str(data_dir)
        env["PANORAMA_LISTEN"] = f"127.0.0.1:{server_port}"

        server_process = subprocess.Popen([str(server_bin)], env=env)

        ready = wait_for_url(
            f"http://127.0.0.1:{server_port}/api/plugins",
            attempts=60,
            interval=0.5,
            desc="server /api/plugins",
        )
        if not ready:
            sys.exit(1)

        click.echo(
            f"  ✓ Server ready on port {server_port} (pid {server_process.pid})"
        )
        click.echo("")

        # Run E2E tests
        click.echo("--- Running E2E tests ---")
        click.echo("")

        playwright_env = env.copy()
        playwright_env["PLAYWRIGHT_BASE_URL"] = f"http://127.0.0.1:{server_port}"

        cmd = ["npx", "playwright", "test", "--project=chromium"] + list(
            playwright_args
        )
        res = subprocess.run(cmd, cwd=repo_root / "frontend", env=playwright_env)

        click.echo("")
        click.echo("=== Tests complete ===")
        sys.exit(res.returncode)

    finally:
        if server_process is not None:
            try:
                server_process.terminate()
                server_process.wait(timeout=5.0)
            except Exception:
                try:
                    server_process.kill()
                except Exception:
                    pass
        temp_dir_obj.cleanup()


if __name__ == "__main__":
    main()
