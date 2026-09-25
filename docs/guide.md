# Corrode Guide

Detailed documentation for running and operating Corrode.

## Table of contents

- [Configuration](#configuration)
- [Student roster](#student-roster)
- [Repository naming](#repository-naming)
- [Review pipeline](#review-pipeline)
- [TUI usage](#tui-usage)
- [Database](#database)
- [Operational notes](#operational-notes)

## Configuration

Copy `Config.example.toml` to `Config.toml`. The file is loaded from the current working directory at startup.

| Section | Key | Required | Default | Description |
| --- | --- | --- | --- | --- |
| `app` | `poll_interval_secs` | yes | — | Delay between polling iterations in continuous mode. |
| `github` | `token` | yes | — | GitHub personal access token. |
| `github` | `organization` | yes | — | GitHub organization to watch. |
| `openai` | `api_key` | yes | — | OpenAI-compatible API key. |
| `openai` | `model` | yes | — | Model name, e.g. `gpt-4o-mini`. |
| `openai` | `base_url` | no | `https://api.openai.com/v1` | Base URL of an OpenAI-compatible endpoint. |
| `db` | `url` | yes | — | SQLite connection string, e.g. `sqlite://corrode.db`. The file is created if missing. |
| `db` | `students_csv` | no | `students.csv` | Path to the student roster CSV (see below). |

## Student roster

The roster CSV is a Google Sheets-style export with a header row and three columns:

```csv
Name,Surname,GitHub
Alice,Smith,alice-smith
Bob,Johnson,@bobjohnson
```

- GitHub handles are normalized: a leading `@` and a trailing `/` are stripped, and full profile URLs are reduced to the login.
- Rows with an empty required column are skipped.
- Import runs at startup and can be reloaded from the TUI Settings screen (`r`).
- The roster is stored in SQLite and refreshed on conflict.

## Repository naming

Corrode derives assignments from repository names. A repository must match the pattern:

```
<assignment-name>-<student-github-login>
```

Example: for a student with login `alice-smith` and assignment `lab1`, the repository is `lab1-alice-smith`. Matching is case-insensitive. Repositories that do not match any student are ignored.

This mapping is persisted in SQLite so submissions can be attributed to a student and assignment.

## Review pipeline

For every open, non-draft PR in the organization (optionally filtered to one student):

1. **Discover** — list open PRs (paginated, 100 per page).
2. **Checkout** — clone the PR head repository at the exact commit SHA over SSH into a temporary directory, then read the changed text files (files larger than 200 KB or a total of 1 MB are skipped; binary files are ignored). The checkout is removed afterwards.
3. **Review** — send the PR title, description, GitHub diff, and the checked-out sources to the model. The model returns a JSON verdict: `summary`, `decision`, and `comments[]` with `file`, `line`, `severity` (`low`/`medium`/`high`/`critical`), and `body`.
4. **Post** — submit the review to GitHub with inline comments. If GitHub rejects it (a comment points to a line outside the diff), Corrode retries with a summary-only review so the verdict is still delivered.
5. **Record** — save the review and submission state in SQLite.

Failures are logged and retried on the next iteration. A PR is re-reviewed only if a new commit arrives.

## TUI usage

Corrode requires an interactive terminal. Logs are written to `corrode.log` (set `RUST_LOG` to adjust the level, e.g. `RUST_LOG=debug`).

| Key | Action |
| --- | --- |
| `j` / `k` or arrows | move selection |
| `Enter` | confirm / run selected action |
| `Esc` / `q` | back or quit |
| `r` (Students) | reload students from disk |
| `r` (Settings) | re-import the roster CSV |

Main menu:

| Item | Action |
| --- | --- |
| Run all repositories once | one polling iteration, then return |
| Run all repositories continuously | poll forever until `q` |
| Run one student | pick a student, review only their repositories |
| Database | show counts: students, assignments, repository mappings, submissions |
| Settings | show current config, reload roster |
| Quit | exit |

## Database

SQLite file (path from `db.url`), created on first run:

- `students` — name, surname, unique GitHub login.
- `assignments` — assignment names.
- `student_repositories` — repository → student/assignment mapping.
- `submissions` — per-student per-assignment commit SHAs with status.
- `reviews` — dedup table keyed by `(repository, pr_number, commit_sha)`.

## Operational notes

- **Token scopes** — the GitHub token needs read access to the organization's repositories and write permission to submit pull request reviews. Fine-grained tokens work; select the required repositories and permissions when creating the token.
- **SSH** — cloning uses SSH (`git@github.com`), so the host must have an SSH key authorized for the GitHub account and `git` available on `PATH`.
- **Rate limits** — API calls use `per_page=100` and retry with backoff on transient failures. The default polling interval of 60 s is conservative.
- **Review output** — the model is instructed to return only valid JSON and never to invent files, lines, or behavior; anything without a safe location goes into the summary.
