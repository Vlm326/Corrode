# Corrode

[![License: MIT](https://img.shields.io/badge/license-MIT-green)](LICENSE)
[![Language: Rust](https://img.shields.io/badge/language-Rust-orange)](Cargo.toml)

Open-source AI homework checker for GitHub Classroom-style courses. Corrode watches every repository in a GitHub organization, finds open pull requests, sends the diffs to an LLM for review, and posts structured feedback directly on the PR — so students get feedback without waiting for a human.

## How it works

1. **Poll** — on a configurable interval, lists all repositories in the configured GitHub organization and all open PRs in each.
2. **Review** — sends the PR title, description, and file diffs to an OpenAI-compatible API, which returns a structured verdict: summary, decision (`approve` / `request_changes` / `comment`), and per-line comments with severity.
3. **Post** — submits the review back to GitHub (`APPROVE` / `REQUEST_CHANGES` / `COMMENT`) and records the result in SQLite, so a commit is never reviewed twice.

## Why Rust

Built in Rust, so it's fast, reliable, and single-binary deployable — it polls GitHub continuously and reviews PRs without eating your resources.

## Configuration

Copy `Config.example.toml` to `Config.toml` and fill in your GitHub token and OpenAI API key:

```toml
[github]
token = "ghp_xxx"        # GitHub personal access token
organization = "org"     # GitHub organization to watch

[openai]
api_key = "sk-xxx"       # OpenAI API key
model = "gpt-4o-mini"
base_url = "https://api.openai.com/v1" # optional, defaults to OpenAI
```

`Config.toml` is git-ignored — only the example is committed.