# Corrode

[![License: MIT](https://img.shields.io/badge/license-MIT-green)](LICENSE)
[![Language: Rust](https://img.shields.io/badge/language-Rust-orange)](Cargo.toml)

Corrode is a production-grade AI code reviewer for GitHub Classroom-style courses. It watches every repository in a GitHub organization, reviews open pull requests with an LLM, and posts structured feedback directly on the PR.

## Features

- **Organization-wide** — polls all repositories and open PRs in the configured GitHub organization.
- **Structured LLM reviews** — a `summary`, a decision (`approve` / `request_changes` / `comment`), and per-line comments with severity.
- **Ground-truth review** — checks out the exact commit via SSH and reads the changed source files, so reviews are grounded in real code, not just the diff.
- **Inline comments** — posts review comments on the diff; if a line falls outside the diff, gracefully falls back to a summary-only review.
- **Submission tracking** — maps repositories to students via a roster CSV and records every submission in SQLite.
- **Dedup and retries** — a commit is never reviewed twice; GitHub API calls retry with backoff.
- **Interactive TUI** — run a one-shot poll, run continuously, review a single student, or inspect database stats.

## Quick start

1. Copy the config template and fill it in:

   ```sh
   cp Config.example.toml Config.toml
   ```

   ```toml
   [github]
   token = "ghp_xxx"      # GitHub token with org access
   organization = "org"   # GitHub organization to watch

   [openai]
   api_key = "sk-xxx"     # OpenAI-compatible API key
   model = "gpt-4o-mini"
   ```

2. Build and run:

   ```sh
   cargo build --release
   ./target/release/corrode
   ```

`Config.toml` is git-ignored — only `Config.example.toml` is committed. See [the guide](docs/guide.md) for the full configuration reference, roster format, and operational notes.

## Requirements

- Rust (edition 2024)
- A GitHub token with read access to the organization and permission to submit PR reviews
- An SSH key on the host for cloning student repositories
- An OpenAI-compatible API key

## How it works

```
GitHub org ──► repositories ──► open PRs ──► checkout @ commit SHA ──► LLM review ──► inline comments + verdict
                       │                                             │                 │
                       └──────────► roster CSV ──► SQLite ───────────┴─────────────────┘
```

## Project layout

```
src/
  main.rs      entry point, file logging
  tui.rs       interactive terminal UI
  app.rs       polling orchestration and submission tracking
  github.rs    GitHub API client (retries, pagination, reviews)
  reviewer.rs  LLM review client and prompt
  runner.rs    git checkout and source extraction
  db.rs        SQLite schema and queries
  csv.rs       student roster parser
  models.rs    shared data structures
```

## License

[MIT](LICENSE)