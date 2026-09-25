#!/usr/bin/env python3
"""Builds an asciinema v2 cast by running the real pas-agent binary in a scratch repo.

Regenerate assets/demo.gif (needs https://github.com/asciinema/agg):

    cargo build --release
    python3 assets/make_demo.py target/release/pas-agent /tmp/pas-demo/demo.cast
    agg --theme github-dark --font-size 16 --idle-time-limit 3.5 --last-frame-duration 4 \
        /tmp/pas-demo/demo.cast assets/demo.gif
"""
import json, os, shutil, subprocess, sys

BIN = sys.argv[1]
OUT = sys.argv[2]
work = os.path.join(os.path.dirname(OUT), "my-app")
shutil.rmtree(work, ignore_errors=True)
os.makedirs(os.path.join(work, "src"))
g = ["git", "-c", "user.name=demo", "-c", "user.email=demo@example.com", "-c", "commit.gpgsign=false"]
subprocess.run(["git", "init", "-q", "-b", "main"], cwd=work, check=True)
open(os.path.join(work, "src/auth.rs"), "w").write("// auth\n")
open(os.path.join(work, "CLAUDE.md"), "w").write("# Project rules\nRun cargo test before committing.\n")
subprocess.run(g + ["add", "."], cwd=work, check=True)
subprocess.run(g + ["commit", "-q", "-m", "init"], cwd=work, check=True)
open(os.path.join(work, "src/auth.rs"), "a").write("fn login() {}\n")
open(os.path.join(work, "src/token.rs"), "w").write("// token\n")

AMBER, DIM, RESET, BOLD = "\x1b[38;5;214m", "\x1b[38;5;245m", "\x1b[0m", "\x1b[1m"
events, t = [], 0.4

def emit(text, dt=0.0):
    global t
    t += dt
    events.append([round(t, 3), "o", text])

def comment(text):
    emit(f"{DIM}# {text}{RESET}\r\n", 0.3)

def run(cmd, show=None, after=1.4):
    emit(f"{AMBER}${RESET} ", 0.35)
    for ch in (show or cmd):
        emit(ch, 0.035)
    emit("\r\n", 0.3)
    env = dict(os.environ, NO_COLOR="1")
    out = subprocess.run(cmd, shell=True, cwd=work, capture_output=True, text=True, env=env)
    text = (out.stdout + out.stderr).replace(work, "~/my-app")
    for line in text.splitlines():
        emit(line + "\r\n", 0.04)
    emit("", after)

B = BIN
comment("Working in Claude Code. Start a session for the task.")
run(f'{B} init --task "Add JWT auth" | head -2', show='pas-agent init --task "Add JWT auth"')
comment("Record progress as you go")
run(f'{B} update --completed "Login endpoint" --remaining "Token refresh" --decision "Use jose for edge runtime"',
    show='pas-agent update --completed "Login endpoint" \\\r\n    --remaining "Token refresh" --decision "Use jose for edge runtime"')
run(f'{B} checkpoint "Login flow done"', show='pas-agent checkpoint "Login flow done"')
comment("Switching to Codex: hand off the context")
run(f"{B} export --to codex", show="pas-agent export --to codex", after=0.8)
run("head -n 22 AGENTS.md", after=3.5)

header = {"version": 2, "width": 92, "height": 30, "env": {"TERM": "xterm-256color", "SHELL": "/bin/bash"}}
with open(OUT, "w") as f:
    f.write(json.dumps(header) + "\n")
    for e in events:
        f.write(json.dumps(e) + "\n")
print(f"{len(events)} events, {t:.1f}s")
