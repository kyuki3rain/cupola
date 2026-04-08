#!/usr/bin/env python3
"""Cupola E2E runner (step-by-step, per-phase).

Design goals:
- One file, stdlib only.
- Each phase is a pure function on a Context; run one or all.
- Ephemeral repo per invocation unless --reuse-repo.
- DB is queried directly (single source of truth) with poll-until-timeout helpers.
- Every check records (id, status, seconds, message) into result.json.

Usage:
    ./cupola_e2e.py preflight                # sanity-check env + build
    ./cupola_e2e.py phase phase_0            # run a single phase
    ./cupola_e2e.py phase phase_1 --reuse-repo owner/cupola-e2e-XXXX
    ./cupola_e2e.py run --to phase_3         # run phases 0..3 on a fresh repo
    ./cupola_e2e.py run                      # run all phases on a fresh repo
    ./cupola_e2e.py delete-repo owner/...    # delete a leftover repo
    ./cupola_e2e.py sweep                    # delete all cupola-e2e-* repos
"""

from __future__ import annotations

import argparse
import json
import os
import random
import shutil
import signal
import sqlite3
import string
import subprocess
import sys
import time
from dataclasses import dataclass, field
from datetime import datetime
from pathlib import Path
from typing import Callable, Optional

SCRIPT_DIR = Path(__file__).resolve().parent
REPO_ROOT = SCRIPT_DIR.parent.parent
SEED_DIR = SCRIPT_DIR / "seed"
FIXTURE_TOML = SCRIPT_DIR / "fixtures" / "cupola.toml.tmpl"
FAKE_CLAUDE = SCRIPT_DIR / "fake-claude-fail.sh"

# ----------------------------------------------------------------------------- #
# Logging                                                                       #
# ----------------------------------------------------------------------------- #

USE_COLOR = sys.stderr.isatty()


def _c(code: str, msg: str) -> str:
    return f"\033[{code}m{msg}\033[0m" if USE_COLOR else msg


def log_info(msg: str) -> None:
    ts = datetime.now().strftime("%H:%M:%S")
    print(f"{_c('36', ts)} [INFO]  {msg}", file=sys.stderr, flush=True)


def log_warn(msg: str) -> None:
    ts = datetime.now().strftime("%H:%M:%S")
    print(f"{_c('33', ts)} [WARN]  {_c('33', msg)}", file=sys.stderr, flush=True)


def log_error(msg: str) -> None:
    ts = datetime.now().strftime("%H:%M:%S")
    print(f"{_c('31', ts)} [ERROR] {_c('31', msg)}", file=sys.stderr, flush=True)


def log_section(title: str) -> None:
    print(f"\n{_c('35', f'=== {title} ===')}\n", file=sys.stderr, flush=True)


# ----------------------------------------------------------------------------- #
# Shell                                                                         #
# ----------------------------------------------------------------------------- #


@dataclass
class RunResult:
    rc: int
    stdout: str
    stderr: str


def sh(
    cmd: list[str] | str,
    *,
    cwd: Optional[Path] = None,
    env: Optional[dict] = None,
    check: bool = False,
    timeout: Optional[float] = None,
    input: Optional[str] = None,
) -> RunResult:
    """Run a shell command. `cmd` is a list (exec) or str (shell=True)."""
    shell = isinstance(cmd, str)
    # Close stdin so interactive prompts (clap's confirm, gh's pager, etc.)
    # never block. Any command that actually needs stdin should pass `input=`.
    stdin = None if input is not None else subprocess.DEVNULL
    proc = subprocess.run(
        cmd,
        cwd=str(cwd) if cwd else None,
        env=env,
        shell=shell,
        stdin=stdin,
        capture_output=True,
        text=True,
        timeout=timeout,
        input=input,
    )
    result = RunResult(rc=proc.returncode, stdout=proc.stdout, stderr=proc.stderr)
    if check and result.rc != 0:
        raise RuntimeError(
            f"command failed (rc={result.rc}): {cmd}\n"
            f"stdout: {result.stdout}\nstderr: {result.stderr}"
        )
    return result


def sh_ok(cmd: list[str] | str, **kwargs) -> str:
    """Run, require rc=0, return stdout stripped."""
    r = sh(cmd, check=True, **kwargs)
    return r.stdout.strip()


# ----------------------------------------------------------------------------- #
# Context + result tracking                                                     #
# ----------------------------------------------------------------------------- #


@dataclass
class Context:
    run_dir: Path
    target_dir: Path
    cupola_bin: Path
    repo_owner: str
    repo_name: str
    repo_full: str  # owner/name
    results: list[dict] = field(default_factory=list)

    @property
    def db_path(self) -> Path:
        return self.target_dir / ".cupola" / "cupola.db"

    # -- DB helpers ------------------------------------------------------- #

    def query_one(self, sql: str, *params) -> Optional[tuple]:
        with sqlite3.connect(str(self.db_path)) as conn:
            cur = conn.execute(sql, params)
            return cur.fetchone()

    def query_all(self, sql: str, *params) -> list[tuple]:
        with sqlite3.connect(str(self.db_path)) as conn:
            cur = conn.execute(sql, params)
            return cur.fetchall()

    # -- Result tracking -------------------------------------------------- #

    def record(self, cp_id: str, status: str, seconds: float, message: str) -> None:
        self.results.append(
            {
                "id": cp_id,
                "status": status,
                "seconds": round(seconds, 2),
                "message": message,
            }
        )
        result_file = self.run_dir / "result.json"
        result_file.write_text(
            json.dumps({"scenarios": self.results}, indent=2, ensure_ascii=False)
        )

    def check(self, cp_id: str, desc: str, fn: Callable[[], None]) -> bool:
        """Run fn(); exception = fail, else pass. Returns True on pass."""
        t0 = time.time()
        try:
            fn()
            elapsed = time.time() - t0
            log_info(f"PASS {cp_id}: {desc} ({elapsed:.1f}s)")
            self.record(cp_id, "pass", elapsed, "ok")
            return True
        except Exception as e:
            elapsed = time.time() - t0
            log_error(f"FAIL {cp_id}: {desc} ({elapsed:.1f}s) — {e}")
            self.record(cp_id, "fail", elapsed, str(e))
            return False

    def skip(self, cp_id: str, desc: str, reason: str) -> None:
        log_info(f"SKIP {cp_id}: {desc} ({reason})")
        self.record(cp_id, "skipped", 0, reason)

    # -- cupola wrapper --------------------------------------------------- #

    def cupola(self, *args: str, timeout: Optional[float] = 300) -> RunResult:
        return sh(
            [str(self.cupola_bin), *args], cwd=self.target_dir, timeout=timeout
        )

    def cupola_ok(self, *args: str, timeout: Optional[float] = 300) -> str:
        r = self.cupola(*args, timeout=timeout)
        if r.rc != 0:
            raise RuntimeError(
                f"cupola {' '.join(args)} failed (rc={r.rc}): "
                f"stdout={r.stdout!r} stderr={r.stderr!r}"
            )
        return r.stdout


# ----------------------------------------------------------------------------- #
# Waiters                                                                       #
# ----------------------------------------------------------------------------- #


def wait_until(
    pred: Callable[[], bool],
    *,
    timeout: float,
    interval: float = 2.0,
    desc: str = "",
) -> None:
    start = time.time()
    while time.time() - start < timeout:
        try:
            if pred():
                return
        except Exception:
            pass
        time.sleep(interval)
    raise TimeoutError(f"timeout after {timeout:.0f}s waiting for: {desc}")


class UnexpectedTerminalState(RuntimeError):
    """Raised when waiting for a state but the issue has already hit a terminal
    state (cancelled/completed) that can't transition back to the desired one.
    Lets waiters fail fast instead of burning the full timeout."""


# States that cannot transition to anything a waiter cares about.
# (`cancelled` can still transition to `idle` on reopen, but if you're
#  waiting for something *other than* `idle`, you've lost.)
_TERMINAL_STATES = {"cancelled", "completed"}


def wait_for_state(ctx: Context, issue_number: int, expected: str, timeout: float) -> None:
    log_info(f"Waiting for #{issue_number} -> {expected} (timeout {timeout:.0f}s)")
    last = [""]
    start = time.time()
    while time.time() - start < timeout:
        row = ctx.query_one(
            "SELECT state FROM issues WHERE github_issue_number=?", issue_number
        )
        cur = row[0] if row else ""
        last[0] = cur
        if cur == expected:
            log_info(f"State reached: #{issue_number} -> {expected}")
            return
        if cur in _TERMINAL_STATES and cur != expected and expected != "idle":
            # Dump recent daemon log lines and failed process_run error messages
            # so the operator can see why without waiting out the timeout.
            _dump_failure_context(ctx, issue_number)
            raise UnexpectedTerminalState(
                f"#{issue_number} is {cur!r} (terminal); will never reach {expected!r}"
            )
        time.sleep(2)
    raise TimeoutError(
        f"#{issue_number} never reached {expected} in {timeout:.0f}s (last: {last[0]!r})"
    )


def _dump_failure_context(ctx: Context, issue_number: int) -> None:
    """Log the latest failed process_runs for the issue + tail of daemon log."""
    try:
        rows = ctx.query_all(
            "SELECT id, type, state, error_message FROM process_runs "
            "WHERE issue_id=(SELECT id FROM issues WHERE github_issue_number=?) "
            "ORDER BY id",
            issue_number,
        )
        log_error(f"process_runs for #{issue_number}:")
        for r in rows:
            log_error(f"  run_id={r[0]} type={r[1]} state={r[2]} err={r[3]!r}")
        # Point to process-run logs if present
        pr_dir = ctx.target_dir / ".cupola" / "logs" / "process-runs"
        if pr_dir.exists():
            log_error(f"per-run logs: {pr_dir}")
            for f in sorted(pr_dir.iterdir()):
                log_error(f"  {f.name}  ({f.stat().st_size} bytes)")
    except Exception as e:
        log_warn(f"_dump_failure_context: {e}")


def wait_for_pr(
    ctx: Context, issue_number: int, pr_type: str, timeout: float
) -> int:
    """Return PR number once the expected cupola PR is open/merged."""
    suffix = "design" if pr_type == "design" else "main"
    head = f"cupola/issue-{issue_number}/{suffix}"
    log_info(f"Waiting for PR on branch {head} (timeout {timeout:.0f}s)")
    found: list[int] = []

    def pred():
        r = sh(
            [
                "gh",
                "pr",
                "list",
                "--repo",
                ctx.repo_full,
                "--head",
                head,
                "--state",
                "all",
                "--json",
                "number,state",
            ]
        )
        if r.rc != 0:
            return False
        data = json.loads(r.stdout or "[]")
        for pr in data:
            if pr["state"] in ("OPEN", "MERGED"):
                found.append(pr["number"])
                return True
        return False

    wait_until(pred, timeout=timeout, interval=5, desc=f"PR for {head}")
    log_info(f"PR found: #{found[0]}")
    return found[0]


def merge_pr(ctx: Context, issue_number: int, pr_type: str) -> int:
    suffix = "design" if pr_type == "design" else "main"
    head = f"cupola/issue-{issue_number}/{suffix}"
    data = json.loads(
        sh_ok(
            [
                "gh",
                "pr",
                "list",
                "--repo",
                ctx.repo_full,
                "--head",
                head,
                "--json",
                "number",
            ]
        )
        or "[]"
    )
    if not data:
        raise RuntimeError(f"merge_pr: no open PR on {head}")
    pr_num = data[0]["number"]
    log_info(f"Merging PR #{pr_num} on {head}")
    sh_ok(
        [
            "gh",
            "pr",
            "merge",
            str(pr_num),
            "--repo",
            ctx.repo_full,
            "--squash",
            "--delete-branch=false",
        ]
    )
    return pr_num


# ----------------------------------------------------------------------------- #
# Prereqs / build                                                               #
# ----------------------------------------------------------------------------- #


def ensure_prereqs() -> Path:
    """Verify env and build cupola. Return cupola binary path."""
    log_section("Prerequisites")

    # gh
    if shutil.which("gh") is None:
        raise RuntimeError("gh CLI not found")
    log_info(f"gh: {sh_ok(['gh', '--version']).splitlines()[0]}")

    auth = sh(["gh", "auth", "status"])
    if auth.rc != 0:
        raise RuntimeError(f"gh not authenticated:\n{auth.stdout}{auth.stderr}")
    full = auth.stdout + auth.stderr
    if "delete_repo" not in full:
        raise RuntimeError(
            "delete_repo scope required. Run: gh auth refresh -h github.com -s delete_repo"
        )
    log_info("gh auth + delete_repo scope: OK")

    # claude
    if shutil.which("claude") is None:
        log_warn("claude CLI not found (phase 1-3,5 will fail)")
    else:
        log_info(f"claude: {sh(['claude', '--version']).stdout.strip() or 'found'}")

    # git / sqlite3
    for tool in ("git", "sqlite3"):
        if shutil.which(tool) is None:
            raise RuntimeError(f"{tool} not found")

    # Build binary (no-op if fresh)
    has_devbox = shutil.which("devbox") is not None
    has_cargo = shutil.which("cargo") is not None
    if not (has_devbox or has_cargo):
        raise RuntimeError("neither cargo nor devbox available")
    log_info("Building cupola release binary (cargo will no-op if current)")
    build_cmd = (
        ["devbox", "run", "--", "cargo", "build", "--release"]
        if has_devbox
        else ["cargo", "build", "--release"]
    )
    r = sh(build_cmd, cwd=REPO_ROOT, timeout=600)
    if r.rc != 0:
        raise RuntimeError(f"cargo build failed: {r.stderr}")

    cupola_bin = REPO_ROOT / "target" / "release" / "cupola"
    if not cupola_bin.exists():
        raise RuntimeError(f"binary not found: {cupola_bin}")
    log_info(f"cupola_bin: {cupola_bin}")
    return cupola_bin


# ----------------------------------------------------------------------------- #
# Repo lifecycle                                                                #
# ----------------------------------------------------------------------------- #


def gh_current_user() -> str:
    out = json.loads(sh_ok(["gh", "api", "user"]))
    return out["login"]


def create_ephemeral_repo(owner: str, run_dir: Path) -> tuple[str, str, Path]:
    """Create a fresh private repo, seed it, return (owner, name, target_dir)."""
    log_section("Create ephemeral repo")
    suffix = "".join(random.choices(string.ascii_lowercase + string.digits, k=3))
    ts = datetime.now().strftime("%Y%m%d-%H%M%S")
    name = f"cupola-e2e-{ts}-{suffix}"
    full = f"{owner}/{name}"
    log_info(f"Repo: {full}")

    target = run_dir / "target"
    target.mkdir(parents=True, exist_ok=True)

    # Create repo (private, with --add-readme so it has a default branch)
    sh_ok(["gh", "repo", "create", full, "--private", "--add-readme"])

    # Clone it
    sh_ok(["gh", "repo", "clone", full, str(target)])

    # Copy seed files
    for item in SEED_DIR.iterdir():
        dest = target / item.name
        if item.is_dir():
            shutil.copytree(item, dest, dirs_exist_ok=True)
        else:
            shutil.copy2(item, dest)

    # Commit + push
    for cmd in (
        ["git", "add", "."],
        ["git", "-c", "user.email=e2e@local", "-c", "user.name=e2e", "commit", "-m", "init: seed"],
        ["git", "push", "origin", "main"],
    ):
        r = sh(cmd, cwd=target)
        if r.rc != 0:
            raise RuntimeError(f"seed commit step failed: {cmd} -> {r.stderr}")

    # Create labels
    for label, color in (
        ("agent:ready", "0e8a16"),
        ("weight:light", "c5def5"),
        ("weight:heavy", "b60205"),
    ):
        sh(
            [
                "gh",
                "label",
                "create",
                label,
                "--repo",
                full,
                "--color",
                color,
                "--force",
            ]
        )
    log_info("Repo seeded + labels created")

    (run_dir / "repo-name.txt").write_text(full)
    return owner, name, target


def delete_repo(full: str) -> None:
    sh(["gh", "repo", "delete", full, "--yes"])


def sweep_e2e_repos(owner: str) -> None:
    log_section("Sweep cupola-e2e-* repos")
    data = json.loads(
        sh_ok(["gh", "repo", "list", owner, "--limit", "1000", "--json", "name"])
    )
    for repo in data:
        if repo["name"].startswith("cupola-e2e-"):
            full = f"{owner}/{repo['name']}"
            log_info(f"Deleting {full}")
            delete_repo(full)


# ----------------------------------------------------------------------------- #
# Cupola init                                                                   #
# ----------------------------------------------------------------------------- #


def init_cupola(ctx: Context) -> None:
    log_section("Initialize cupola")
    r = ctx.cupola("init")
    if r.rc != 0:
        raise RuntimeError(f"cupola init failed: {r.stderr}")
    log_info("cupola init: OK")

    # Render cupola.toml from template
    tmpl = FIXTURE_TOML.read_text()
    rendered = tmpl.replace("__OWNER__", ctx.repo_owner).replace(
        "__REPO__", ctx.repo_name
    )
    (ctx.target_dir / ".cupola" / "cupola.toml").write_text(rendered)
    log_info("cupola.toml rendered")

    # Seed placeholder steering file so doctor passes
    steering = ctx.target_dir / ".cupola" / "steering"
    steering.mkdir(parents=True, exist_ok=True)
    if not any(steering.iterdir()):
        (steering / "product.md").write_text(
            "# Product (E2E placeholder)\n\nSeeded by cupola_e2e.py.\n"
        )
        log_info("steering/product.md seeded")

    r = ctx.cupola("doctor")
    if r.rc != 0:
        raise RuntimeError(f"cupola doctor failed:\n{r.stdout}")
    log_info("cupola doctor: OK")


# ----------------------------------------------------------------------------- #
# Daemon lifecycle helpers                                                      #
# ----------------------------------------------------------------------------- #


def daemon_start(ctx: Context) -> None:
    r = ctx.cupola("start", "--daemon")
    if r.rc != 0:
        raise RuntimeError(f"start --daemon failed: {r.stderr}")
    # Confirm via status
    time.sleep(1)


def daemon_stop(ctx: Context) -> None:
    ctx.cupola("stop")


def daemon_is_running(ctx: Context) -> bool:
    r = ctx.cupola("status")
    return "Daemon: running" in r.stdout


# ----------------------------------------------------------------------------- #
# Issue helpers                                                                 #
# ----------------------------------------------------------------------------- #


def create_issue(
    ctx: Context, title: str, body: str, labels: list[str]
) -> int:
    cmd = ["gh", "issue", "create", "--repo", ctx.repo_full, "--title", title, "--body", body]
    for lbl in labels:
        cmd += ["--label", lbl]
    out = sh_ok(cmd)
    # stdout is the URL ".../issues/N"
    url = out.strip().splitlines()[-1]
    return int(url.rsplit("/", 1)[-1])


def db_issue_id(ctx: Context, github_num: int) -> int:
    row = ctx.query_one(
        "SELECT id FROM issues WHERE github_issue_number=?", github_num
    )
    if not row:
        raise RuntimeError(f"no DB row for #{github_num}")
    return int(row[0])


# ----------------------------------------------------------------------------- #
# Phase 0: Pre-flight                                                           #
# ----------------------------------------------------------------------------- #


def phase_0_preflight(ctx: Context) -> None:
    log_section("Phase 0: Pre-flight")

    def cp00():
        r = ctx.cupola("doctor")
        if r.rc != 0:
            raise RuntimeError(f"doctor exit {r.rc}: {r.stdout}")
        if "Start Readiness" not in r.stdout:
            raise RuntimeError("missing 'Start Readiness' section")
        if "❌" in r.stdout:
            raise RuntimeError("doctor output contains ❌")

    ctx.check("CP-00", "cupola doctor exit 0, Start Readiness present, no ❌", cp00)

    # Write user markers to verify init --upgrade preserves them
    toml_path = ctx.target_dir / ".cupola" / "cupola.toml"
    toml_path.write_text(toml_path.read_text() + "\n# user-marker\n")
    dummy = ctx.target_dir / ".cupola" / "steering" / "dummy.md"
    dummy.write_text("dummy steering\n")

    def cp01():
        ctx.cupola("init", "--upgrade")
        if "# user-marker" not in toml_path.read_text():
            raise RuntimeError("user-marker missing from cupola.toml")
        if not dummy.exists():
            raise RuntimeError("dummy.md was removed")

    ctx.check("CP-01", "init --upgrade preserves cupola.toml marker and dummy.md", cp01)

    def cp02():
        db = ctx.target_dir / ".cupola" / "cupola.db"
        if not db.exists() or db.stat().st_size == 0:
            raise RuntimeError("cupola.db missing or empty")

    ctx.check("CP-02", ".cupola/cupola.db exists after --upgrade", cp02)

    def cp03():
        r = ctx.cupola("start", "--daemon")
        if r.rc != 0:
            raise RuntimeError(f"start --daemon exit {r.rc}: {r.stderr}")
        import re

        if not re.search(r"started cupola daemon \(pid=\d+\)", r.stdout):
            raise RuntimeError(f"pattern not found: {r.stdout!r}")

    ctx.check("CP-03", "start --daemon stdout matches pattern", cp03)

    def cp04():
        r = ctx.cupola("start", "--daemon")
        if r.rc == 0:
            raise RuntimeError("expected non-zero exit")
        combined = r.stdout + r.stderr
        if "already running" not in combined.lower():
            raise RuntimeError(f"missing 'already running': {combined!r}")

    ctx.check("CP-04", "second start --daemon fails with 'already running'", cp04)

    def cp05():
        import re

        r = ctx.cupola("status")
        if not re.search(r"Daemon: running \(pid=\d+\)", r.stdout):
            raise RuntimeError(f"pattern not found: {r.stdout!r}")

    ctx.check("CP-05", "status contains 'Daemon: running (pid=<N>)'", cp05)

    def cp06():
        r = ctx.cupola("stop")
        if "stopped cupola" not in r.stdout:
            raise RuntimeError(f"missing 'stopped cupola': {r.stdout!r}")

    ctx.check("CP-06", "stop contains 'stopped cupola'", cp06)

    def cp07():
        r = ctx.cupola("status")
        if "Daemon: not running" not in r.stdout:
            raise RuntimeError(f"missing 'Daemon: not running': {r.stdout!r}")

    ctx.check("CP-07", "status after stop contains 'Daemon: not running'", cp07)

    def cp08():
        pid_file = ctx.target_dir / ".cupola" / "cupola.pid"
        if pid_file.exists():
            raise RuntimeError("cupola.pid still exists after stop")

    ctx.check("CP-08", ".cupola/cupola.pid does not exist after stop", cp08)

    # Restart daemon for subsequent phases
    log_info("Restarting daemon for subsequent phases")
    daemon_start(ctx)


# ----------------------------------------------------------------------------- #
# Phase 1: Happy path                                                           #
# ----------------------------------------------------------------------------- #


def _ensure_daemon(ctx: Context) -> None:
    if not daemon_is_running(ctx):
        log_info("daemon not running, starting...")
        daemon_start(ctx)


def phase_1_happy_path(ctx: Context) -> None:
    log_section("Phase 1: Happy path")
    _ensure_daemon(ctx)

    issue_n = None

    def cp10():
        nonlocal issue_n
        issue_n = create_issue(
            ctx,
            "E2E Phase 1: add FOO to README",
            'README.md に "FOO" という一行を追加してください。',
            ["weight:light", "agent:ready"],
        )
        log_info(f"ISSUE_1={issue_n}")

    ctx.check("CP-10", "Create issue #1", cp10)
    assert issue_n is not None

    ctx.check("CP-11", f"#{issue_n} reaches initialize_running (180s)",
              lambda: wait_for_state(ctx, issue_n, "initialize_running", 180))
    ctx.check("CP-12", f"#{issue_n} reaches design_running (300s)",
              lambda: wait_for_state(ctx, issue_n, "design_running", 300))

    def cp13():
        r = ctx.cupola("status")
        if f"#{issue_n}" not in r.stdout or "design_running" not in r.stdout:
            raise RuntimeError(f"status missing row: {r.stdout!r}")

    ctx.check("CP-13", f"status shows #{issue_n} in design_running", cp13)

    pr_design = None

    def cp15():
        nonlocal pr_design
        pr_design = wait_for_pr(ctx, issue_n, "design", 1200)
        row = ctx.query_one(
            "SELECT pr_number FROM process_runs "
            "WHERE issue_id=(SELECT id FROM issues WHERE github_issue_number=?) "
            "  AND type='design' ORDER BY id DESC LIMIT 1",
            issue_n,
        )
        if not row or row[0] != pr_design:
            raise RuntimeError(f"DB pr_number={row[0] if row else None} != {pr_design}")

    ctx.check("CP-15", f"#{issue_n} design PR open + pr_number in DB", cp15)

    ctx.check("CP-16", f"Merge design PR",
              lambda: merge_pr(ctx, issue_n, "design"))

    ctx.check("CP-17", f"#{issue_n} reaches implementation_running (300s)",
              lambda: wait_for_state(ctx, issue_n, "implementation_running", 300))

    def cp18():
        pr = wait_for_pr(ctx, issue_n, "impl", 1200)
        log_info(f"PR_1_IMPL={pr}")

    ctx.check("CP-18", f"#{issue_n} impl PR open", cp18)

    ctx.check("CP-19", f"Merge impl PR",
              lambda: merge_pr(ctx, issue_n, "impl"))

    ctx.check("CP-20", f"#{issue_n} reaches completed (300s)",
              lambda: wait_for_state(ctx, issue_n, "completed", 300))

    def cp21():
        wt = ctx.target_dir / ".cupola" / "worktrees" / f"issue-{issue_n}"
        wait_until(lambda: not wt.exists(), timeout=180, desc="worktree removed")

    ctx.check("CP-21", f"worktree removed (retry)", cp21)

    def cp22():
        def pred():
            row = ctx.query_one(
                "SELECT close_finished FROM issues WHERE github_issue_number=?",
                issue_n,
            )
            return row and row[0] == 1
        wait_until(pred, timeout=180, desc="close_finished=1")

    ctx.check("CP-22", f"DB close_finished=1 for #{issue_n} (retry)", cp22)

    def cp23():
        data = json.loads(
            sh_ok(
                [
                    "gh", "issue", "view", str(issue_n),
                    "--repo", ctx.repo_full,
                    "--json", "comments",
                ]
            )
        )
        if len(data["comments"]) < 1:
            raise RuntimeError("no comments on issue")

    ctx.check("CP-23", f"Issue #{issue_n} has at least 1 comment", cp23)


# ----------------------------------------------------------------------------- #
# Phase 2: PR close recovery                                                    #
# ----------------------------------------------------------------------------- #


def phase_2_pr_close_recovery(ctx: Context) -> None:
    log_section("Phase 2: PR close recovery")
    _ensure_daemon(ctx)

    issue_n = None
    old_pr = None
    new_pr = None

    def cp30():
        nonlocal issue_n
        issue_n = create_issue(
            ctx, "E2E Phase 2: add BAR to README",
            'README.md に "BAR" という一行を追加してください。',
            ["weight:light", "agent:ready"],
        )
        log_info(f"ISSUE_2={issue_n}")

    ctx.check("CP-30", "Create issue #2", cp30)
    assert issue_n is not None

    def cp31():
        nonlocal old_pr
        wait_for_state(ctx, issue_n, "design_review_waiting", 1200)
        old_pr = wait_for_pr(ctx, issue_n, "design", 60)
        log_info(f"OLD_PR={old_pr}")

    ctx.check("CP-31", f"#{issue_n} design_review_waiting + PR open", cp31)

    ctx.check("CP-32", f"Close PR #{old_pr} without merge",
              lambda: sh_ok([
                  "gh", "pr", "close", str(old_pr),
                  "--repo", ctx.repo_full, "--delete-branch=false",
              ]))

    ctx.check("CP-33", f"#{issue_n} returns to design_running (300s)",
              lambda: wait_for_state(ctx, issue_n, "design_running", 300))

    def cp34():
        nonlocal new_pr
        new_pr = wait_for_pr(ctx, issue_n, "design", 1200)
        if new_pr == old_pr:
            raise RuntimeError(f"new PR same as old: {new_pr}")

    ctx.check("CP-34", f"New design PR (!= OLD_PR {old_pr})", cp34)

    def cp35():
        row = ctx.query_one(
            "SELECT pr_number FROM process_runs "
            "WHERE issue_id=(SELECT id FROM issues WHERE github_issue_number=?) "
            "  AND type='design' ORDER BY id DESC LIMIT 1",
            issue_n,
        )
        if not row or row[0] != new_pr:
            raise RuntimeError(f"DB pr_number={row[0] if row else None} != {new_pr}")

    ctx.check("CP-35", f"DB latest design pr_number == {new_pr}", cp35)

    def cp36():
        merge_pr(ctx, issue_n, "design")
        wait_for_state(ctx, issue_n, "implementation_running", 300)
        wait_for_pr(ctx, issue_n, "impl", 1200)
        merge_pr(ctx, issue_n, "impl")
        wait_for_state(ctx, issue_n, "completed", 300)

    ctx.check("CP-36", f"Drive #{issue_n} through impl to completed", cp36)


# ----------------------------------------------------------------------------- #
# Phase 3: Cancel + reopen                                                      #
# ----------------------------------------------------------------------------- #


def phase_3_cancel_reopen(ctx: Context) -> None:
    log_section("Phase 3: Cancel + reopen")
    _ensure_daemon(ctx)

    issue_n = None

    def cp40():
        nonlocal issue_n
        issue_n = create_issue(
            ctx, "E2E Phase 3: add BAZ to README",
            'README.md に "BAZ" という一行を追加してください。',
            ["weight:light", "agent:ready"],
        )
        log_info(f"ISSUE_3={issue_n}")
        wait_for_state(ctx, issue_n, "design_running", 600)

    ctx.check("CP-40", "Create issue #3 and reach design_running", cp40)
    assert issue_n is not None

    ctx.check("CP-41", f"Close #{issue_n} with --reason 'not planned'",
              lambda: sh_ok([
                  "gh", "issue", "close", str(issue_n),
                  "--repo", ctx.repo_full, "--reason", "not planned",
              ]))

    ctx.check("CP-42", f"#{issue_n} reaches cancelled (300s)",
              lambda: wait_for_state(ctx, issue_n, "cancelled", 300))

    def cp43():
        data = json.loads(sh_ok([
            "gh", "issue", "view", str(issue_n),
            "--repo", ctx.repo_full, "--json", "comments",
        ]))
        if len(data["comments"]) < 1:
            raise RuntimeError("no comments")

    ctx.check("CP-43", f"Issue has cancel comment", cp43)

    def cp44():
        # Cancelled: worktree should remain (unlike completed)
        wt = ctx.target_dir / ".cupola" / "worktrees" / f"issue-{issue_n}"
        if not wt.exists():
            raise RuntimeError("worktree removed (expected to remain on cancel)")

    ctx.check("CP-44", f"worktree remains after cancel", cp44)

    def cp45():
        def pred():
            row = ctx.query_one(
                "SELECT close_finished FROM issues WHERE github_issue_number=?",
                issue_n,
            )
            return row and row[0] == 1
        wait_until(pred, timeout=120, desc="close_finished=1 after cancel")

    ctx.check("CP-45", f"close_finished=1 after cancel", cp45)

    ctx.check("CP-46", f"Reopen #{issue_n}",
              lambda: sh_ok(["gh", "issue", "reopen", str(issue_n), "--repo", ctx.repo_full]))

    ctx.check("CP-47", f"#{issue_n} reaches idle (180s)",
              lambda: wait_for_state(ctx, issue_n, "idle", 180))
    ctx.check("CP-48", f"#{issue_n} reaches initialize_running (180s)",
              lambda: wait_for_state(ctx, issue_n, "initialize_running", 180))

    def cp49():
        wait_for_state(ctx, issue_n, "design_review_waiting", 1200)
        wait_for_pr(ctx, issue_n, "design", 60)
        merge_pr(ctx, issue_n, "design")
        wait_for_state(ctx, issue_n, "implementation_running", 300)
        wait_for_pr(ctx, issue_n, "impl", 1200)
        merge_pr(ctx, issue_n, "impl")
        wait_for_state(ctx, issue_n, "completed", 300)

    ctx.check("CP-49", f"Drive #{issue_n} to completed", cp49)


# ----------------------------------------------------------------------------- #
# Phase 4: Retry exhaustion + cleanup                                           #
# ----------------------------------------------------------------------------- #


def phase_4_retry_and_cleanup(ctx: Context) -> None:
    log_section("Phase 4: Retry + cleanup")

    saved_path = os.environ.get("PATH", "")
    fake_bin_dir = ctx.run_dir / "fake-bin"
    fake_bin_dir.mkdir(parents=True, exist_ok=True)
    fake_claude = fake_bin_dir / "claude"
    shutil.copy(FAKE_CLAUDE, fake_claude)
    fake_claude.chmod(0o755)

    def cp50():
        ctx.cupola("stop")
        env = os.environ.copy()
        env["PATH"] = f"{fake_bin_dir}:{saved_path}"
        r = sh([str(ctx.cupola_bin), "start", "--daemon"], cwd=ctx.target_dir, env=env)
        if r.rc != 0:
            raise RuntimeError(f"start with fake claude failed: {r.stderr}")

    ctx.check("CP-50", "Inject fake-claude into PATH + restart cupola", cp50)

    issue_n = None

    def cp51():
        nonlocal issue_n
        issue_n = create_issue(
            ctx, "E2E Phase 4: add QUX to README",
            'README.md に "QUX" という一行を追加してください。',
            ["weight:light", "agent:ready"],
        )
        log_info(f"ISSUE_4={issue_n}")

    ctx.check("CP-51", "Create issue #4", cp51)
    assert issue_n is not None

    ctx.check("CP-52", f"#{issue_n} reaches cancelled (retry exhausted, 900s)",
              lambda: wait_for_state(ctx, issue_n, "cancelled", 900))

    def cp53():
        data = json.loads(sh_ok([
            "gh", "issue", "view", str(issue_n),
            "--repo", ctx.repo_full, "--json", "comments",
        ]))
        if len(data["comments"]) < 1:
            raise RuntimeError("no comments")

    ctx.check("CP-53", "Issue has retry-exhausted comment", cp53)

    def cp54():
        row = ctx.query_one(
            "SELECT COUNT(*) FROM process_runs "
            "WHERE issue_id=(SELECT id FROM issues WHERE github_issue_number=?) "
            "  AND state='failed'",
            issue_n,
        )
        if not row or row[0] < 2:
            raise RuntimeError(f"failed count={row[0] if row else 0}")

    ctx.check("CP-54", f"DB has >= 2 failed process_runs for #{issue_n}", cp54)

    # Restore PATH, stop cupola
    ctx.cupola("stop")

    def cp56():
        r = ctx.cupola("cleanup")
        if f"#{issue_n}" not in r.stdout:
            raise RuntimeError(f"cleanup output missing #{issue_n}: {r.stdout!r}")

    ctx.check("CP-56", f"cupola cleanup mentions #{issue_n}", cp56)

    def cp57():
        r = sh(
            ["git", "ls-remote", "origin", f"cupola/issue-{issue_n}/*"],
            cwd=ctx.target_dir,
        )
        if r.stdout.strip():
            raise RuntimeError(f"remote refs still exist: {r.stdout}")

    ctx.check("CP-57", f"Remote cupola/issue-{issue_n}/* deleted", cp57)

    def cp58():
        wt = ctx.target_dir / ".cupola" / "worktrees" / f"issue-{issue_n}"
        if wt.exists():
            raise RuntimeError("worktree still present")

    ctx.check("CP-58", f"worktree removed after cleanup", cp58)

    def cp59():
        row = ctx.query_one(
            "SELECT COUNT(*) FROM process_runs "
            "WHERE issue_id=(SELECT id FROM issues WHERE github_issue_number=?) "
            "  AND pr_number IS NOT NULL",
            issue_n,
        )
        if row and row[0] != 0:
            raise RuntimeError(f"non-null pr_number count={row[0]}")

    ctx.check("CP-59", f"DB all process_runs pr_number=NULL after cleanup", cp59)

    def cp60():
        row = ctx.query_one(
            "SELECT ci_fix_count FROM issues WHERE github_issue_number=?",
            issue_n,
        )
        if not row or row[0] != 0:
            raise RuntimeError(f"ci_fix_count={row[0] if row else None}")

    ctx.check("CP-60", f"DB ci_fix_count=0 for #{issue_n}", cp60)

    # Start daemon again for subsequent phases (with normal PATH)
    def cp62():
        r = ctx.cupola("start", "--daemon")
        if r.rc != 0:
            raise RuntimeError(f"start failed: {r.stderr}")
        time.sleep(1)
        if not daemon_is_running(ctx):
            raise RuntimeError("daemon not running after start")

    ctx.check("CP-62", "cupola start --daemon after cleanup", cp62)


# ----------------------------------------------------------------------------- #
# Phase 5: Orphan recovery                                                      #
# ----------------------------------------------------------------------------- #


def phase_5_orphan_recovery(ctx: Context) -> None:
    log_section("Phase 5: Orphan recovery")
    _ensure_daemon(ctx)

    issue_n = None

    def cp70():
        nonlocal issue_n
        issue_n = create_issue(
            ctx, "E2E Phase 5: add QUUX to README",
            'README.md に "QUUX" という一行を追加してください。',
            ["weight:light", "agent:ready"],
        )
        log_info(f"ISSUE_5={issue_n}")
        wait_for_state(ctx, issue_n, "design_running", 600)

    ctx.check("CP-70", "Create issue #5 and reach design_running", cp70)
    assert issue_n is not None

    db_issue = db_issue_id(ctx, issue_n)
    orig_run_id = None

    def cp71():
        nonlocal orig_run_id
        def pred():
            nonlocal orig_run_id
            row = ctx.query_one(
                "SELECT id FROM process_runs "
                "WHERE issue_id=? AND type='design' AND state='running' "
                "ORDER BY id DESC LIMIT 1",
                db_issue,
            )
            if row:
                orig_run_id = row[0]
                return True
            return False
        wait_until(pred, timeout=30, desc="design running row")
        log_info(f"ORIG_RUN_ID={orig_run_id}")

    ctx.check("CP-71", f"DB design process_run for #{issue_n} is running", cp71)
    assert orig_run_id is not None

    def cp72():
        pid_file = ctx.target_dir / ".cupola" / "cupola.pid"
        pid = int(pid_file.read_text().strip())
        log_info(f"Killing cupola pid={pid}")
        os.kill(pid, signal.SIGKILL)
        time.sleep(1)

    ctx.check("CP-72", "kill -9 cupola daemon", cp72)

    def cp73():
        pid_file = ctx.target_dir / ".cupola" / "cupola.pid"
        if pid_file.exists():
            pid_file.unlink()

    ctx.check("CP-73", "Remove stale cupola.pid", cp73)

    def cp74():
        r = ctx.cupola("start", "--daemon")
        if r.rc != 0:
            raise RuntimeError(f"start failed: {r.stderr}")
        time.sleep(2)

    ctx.check("CP-74", "cupola start --daemon after crash", cp74)

    def cp75():
        def pred():
            row = ctx.query_one(
                "SELECT state, error_message FROM process_runs WHERE id=?",
                orig_run_id,
            )
            if not row:
                return False
            state, msg = row
            return state == "failed" and msg and "orphan" in msg.lower()
        wait_until(pred, timeout=60, desc="orphan recovery marks run failed")

    ctx.check("CP-75", "orphan recovery marks original run as failed/orphaned", cp75)

    def cp76():
        def pred():
            row = ctx.query_one(
                "SELECT COUNT(*) FROM process_runs "
                "WHERE issue_id=? AND id > ? AND state='running'",
                db_issue,
                orig_run_id,
            )
            return row and row[0] >= 1
        wait_until(pred, timeout=180, desc="new running process_run")

    ctx.check("CP-76", f"new process_run for #{issue_n} reaches running", cp76)


# ----------------------------------------------------------------------------- #
# Phase 6/7                                                                     #
# ----------------------------------------------------------------------------- #


def phase_6_compress(ctx: Context) -> None:
    log_section("Phase 6: Compress")

    def cp80():
        # Pull merged content (specs live on remote main, not local clone)
        sh(["git", "pull", "--ff-only", "origin", "main"], cwd=ctx.target_dir)
        specs = ctx.target_dir / ".cupola" / "specs"
        if not specs.exists():
            raise RuntimeError(".cupola/specs not found after pull")
        count = sum(1 for _ in specs.glob("**/spec.json"))
        if count < 1:
            raise RuntimeError(f"no spec.json found (count={count})")

    ctx.check("CP-80", "main branch has .cupola/specs/*/spec.json", cp80)

    def cp81():
        r = ctx.cupola("compress")
        if r.rc != 0:
            raise RuntimeError(f"compress exit {r.rc}: {r.stderr}")

    ctx.check("CP-81", "cupola compress exits 0", cp81)


def phase_7_teardown(ctx: Context) -> None:
    log_section("Phase 7: Teardown")

    def cp99():
        ctx.cupola("stop")
        r = ctx.cupola("status")
        if "not running" not in r.stdout:
            raise RuntimeError(f"status after stop: {r.stdout!r}")

    ctx.check("CP-99", "cupola stop + status shows not running", cp99)


# ----------------------------------------------------------------------------- #
# Phase registry                                                                #
# ----------------------------------------------------------------------------- #


PHASES: dict[str, Callable[[Context], None]] = {
    "phase_0": phase_0_preflight,
    "phase_1": phase_1_happy_path,
    "phase_2": phase_2_pr_close_recovery,
    "phase_3": phase_3_cancel_reopen,
    "phase_4": phase_4_retry_and_cleanup,
    "phase_5": phase_5_orphan_recovery,
    "phase_6": phase_6_compress,
    "phase_7": phase_7_teardown,
}

PHASE_ORDER = list(PHASES.keys())


# ----------------------------------------------------------------------------- #
# Orchestration                                                                 #
# ----------------------------------------------------------------------------- #


def make_run_dir() -> Path:
    base = Path.home() / "work" / "cupola-e2e-run"
    base.mkdir(parents=True, exist_ok=True)
    ts = datetime.now().strftime("%Y%m%d-%H%M%S")
    suffix = "".join(random.choices(string.ascii_lowercase + string.digits, k=3))
    run_dir = base / f"{ts}-{suffix}"
    run_dir.mkdir()
    log_info(f"Run directory: {run_dir}")
    return run_dir


def build_context(
    cupola_bin: Path,
    run_dir: Path,
    reuse_repo: Optional[str],
    owner_override: Optional[str],
) -> Context:
    if reuse_repo:
        owner, name = reuse_repo.split("/", 1)
        target = run_dir / "target"
        if not target.exists():
            sh_ok(["gh", "repo", "clone", reuse_repo, str(target)])
    else:
        owner = owner_override or gh_current_user()
        owner, name, target = create_ephemeral_repo(owner, run_dir)

    ctx = Context(
        run_dir=run_dir,
        target_dir=target,
        cupola_bin=cupola_bin,
        repo_owner=owner,
        repo_name=name,
        repo_full=f"{owner}/{name}",
    )
    init_cupola(ctx)
    return ctx


def summarize(ctx: Context) -> int:
    passed = sum(1 for r in ctx.results if r["status"] == "pass")
    failed = sum(1 for r in ctx.results if r["status"] == "fail")
    skipped = sum(1 for r in ctx.results if r["status"] == "skipped")
    log_section("Results")
    print(f"  Total:   {len(ctx.results)}", file=sys.stderr)
    print(f"  Passed:  {passed}", file=sys.stderr)
    print(f"  Failed:  {failed}", file=sys.stderr)
    print(f"  Skipped: {skipped}", file=sys.stderr)
    print(f"  Result:  {ctx.run_dir / 'result.json'}", file=sys.stderr)
    return 0 if failed == 0 else 1


def run_phases(
    phase_names: list[str],
    *,
    reuse_repo: Optional[str],
    owner: Optional[str],
    keep_repo: bool,
    fail_fast: bool,
) -> int:
    cupola_bin = ensure_prereqs()
    run_dir = make_run_dir()
    ctx = build_context(cupola_bin, run_dir, reuse_repo, owner)

    exit_code = 0
    try:
        for name in phase_names:
            fn = PHASES[name]
            log_section(f"Running {name}")
            try:
                fn(ctx)
                log_info(f"Phase {name}: completed")
            except Exception as e:
                log_error(f"Phase {name}: raised — {e}")
                exit_code = 1
                if fail_fast:
                    break
    finally:
        log_section("Teardown")
        try:
            ctx.cupola("stop")
        except Exception:
            pass
        # Preserve artifacts
        preserved = ctx.run_dir / "preserved"
        preserved.mkdir(exist_ok=True)
        for sub in (".cupola/logs", ".cupola/cupola.db"):
            src = ctx.target_dir / sub
            if src.exists():
                dest = preserved / Path(sub).name
                try:
                    if src.is_dir():
                        shutil.copytree(src, dest, dirs_exist_ok=True)
                    else:
                        shutil.copy2(src, dest)
                except Exception as e:
                    log_warn(f"preserve {sub}: {e}")

        if not reuse_repo and not keep_repo and exit_code == 0:
            log_info(f"Deleting ephemeral repo: {ctx.repo_full}")
            delete_repo(ctx.repo_full)
        else:
            log_warn(f"Repo preserved: https://github.com/{ctx.repo_full}")
            log_warn(f"Run dir:        {ctx.run_dir}")
            log_warn(
                f"Delete later:   ./scripts/e2e/cupola_e2e.py delete-repo {ctx.repo_full}"
            )

    rc = summarize(ctx)
    return rc or exit_code


# ----------------------------------------------------------------------------- #
# CLI                                                                           #
# ----------------------------------------------------------------------------- #


def main() -> int:
    ap = argparse.ArgumentParser(description="Cupola E2E runner (Python)")
    sub = ap.add_subparsers(dest="cmd", required=True)

    p_pre = sub.add_parser("preflight", help="Check env + build binary")

    p_phase = sub.add_parser("phase", help="Run one or more phases")
    p_phase.add_argument(
        "names",
        nargs="+",
        choices=PHASE_ORDER,
        help="phase(s) to run",
    )
    for sp in (p_phase,):
        sp.add_argument("--reuse-repo", default=None, help="owner/name")
        sp.add_argument("--owner", default=None)
        sp.add_argument("--keep-repo", action="store_true")
        sp.add_argument("--fail-fast", action="store_true")

    p_run = sub.add_parser("run", help="Run a range of phases on a fresh repo")
    p_run.add_argument("--from", dest="from_", default=PHASE_ORDER[0], choices=PHASE_ORDER)
    p_run.add_argument("--to", dest="to", default=PHASE_ORDER[-1], choices=PHASE_ORDER)
    p_run.add_argument("--owner", default=None)
    p_run.add_argument("--keep-repo", action="store_true")
    p_run.add_argument("--fail-fast", action="store_true")

    p_del = sub.add_parser("delete-repo", help="Delete a repo by owner/name")
    p_del.add_argument("full")

    sub.add_parser("sweep", help="Delete all cupola-e2e-* repos in the current account")

    args = ap.parse_args()

    if args.cmd == "preflight":
        ensure_prereqs()
        return 0

    if args.cmd == "delete-repo":
        delete_repo(args.full)
        log_info(f"Deleted {args.full}")
        return 0

    if args.cmd == "sweep":
        sweep_e2e_repos(gh_current_user())
        return 0

    if args.cmd == "phase":
        return run_phases(
            args.names,
            reuse_repo=args.reuse_repo,
            owner=args.owner,
            keep_repo=args.keep_repo,
            fail_fast=args.fail_fast,
        )

    if args.cmd == "run":
        i = PHASE_ORDER.index(args.from_)
        j = PHASE_ORDER.index(args.to)
        phases = PHASE_ORDER[i : j + 1]
        return run_phases(
            phases,
            reuse_repo=None,
            owner=args.owner,
            keep_repo=args.keep_repo,
            fail_fast=args.fail_fast,
        )

    return 1


if __name__ == "__main__":
    sys.exit(main())
