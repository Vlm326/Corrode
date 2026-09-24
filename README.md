# Corrode

[![License: MIT](https://img.shields.io/badge/License-MIT-green.svg)](LICENSE)
[![Rust edition: 2024](https://img.shields.io/badge/Rust-edition%202024-orange.svg)](Cargo.toml)

**Corrode** is an automated code review tool for student pull requests on GitHub. It is used in the Rust elective at the Faculty of Mathematics and Mechanics of Saint Petersburg State University (SPbU).

Corrode discovers open pull requests in a GitHub organization, reviews them with an LLM, posts the results and comments directly to each PR, and stores review and course repository records in SQLite. The application is operated through an interactive terminal UI.

## Features

- Reviews open pull requests across all accessible repositories in an organization; draft PRs are skipped.
- Analyzes the diff and source files from the exact commit, cloned from the repository over SSH.
- Uses the assignment statement as review context when a local statement file is available.
- Produces a verdict (`approve`, `request_changes`, or `comment`) and up to five inline comments with severity levels.
- Posts reviews to GitHub. If GitHub rejects inline comments, Corrode retries without them so the overall review can still be published.
- Maps students to course repositories from a CSV roster and stores review and submission records in SQLite.
- Retries transient failures and avoids publishing another review for a commit that has already been reviewed successfully.
- Provides a TUI for one-shot and continuous polling, reviewing one student's repositories, viewing database statistics, and reloading the student roster.

## Contents

- [Requirements](#requirements)
- [Quick start](#quick-start)
- [Configuration](#configuration)
- [Repository naming and student roster](#repository-naming-and-student-roster)
- [Using Corrode](#using-corrode)
- [Review workflow](#review-workflow)
- [Data and privacy](#data-and-privacy)
- [Development](#development)
- [Project layout](#project-layout)
- [License](#license)

## Requirements

- Rust edition 2024 and Cargo.
- Git available on `PATH`.
- An interactive terminal. Corrode launches a TUI and does not support running without a terminal.
- A GitHub Personal Access Token with access to the organization's repositories: permission to read repository metadata and pull requests, and **Pull requests: write** permission to publish reviews. For a fine-grained token, select the required repositories and permissions.
- An SSH key configured on the host with read access to the repositories Corrode will clone.
- An API key for a service compatible with the OpenAI Chat Completions API. Corrode uses the OpenAI API by default; the API endpoint can be changed with `base_url`.

## Quick start

From the project root, copy the example configuration and fill it in:

```sh
cp Config.example.toml Config.toml
```

Set your GitHub token, organization, model API key, and student CSV path. Then build and run Corrode from the project root:

```sh
cargo run --release
```

Alternatively, build the binary and run it directly:

```sh
cargo build --release
./target/release/corrode
```

Corrode reads `Config.toml` from the current working directory. Local configuration, roster CSV, SQLite database, and log files are excluded from Git. The configuration template is [`Config.example.toml`](Config.example.toml); see the [guide](docs/guide.md) for the full configuration reference.

## Configuration

`Config.toml` contains the following sections:

| Section | Key | Description |
| --- | --- | --- |
| `app` | `poll_interval_secs` | Delay between polling iterations in continuous mode, in seconds. |
| `github` | `token` | GitHub Personal Access Token. |
| `github` | `organization` | GitHub organization containing the course repositories. |
| `openai` | `api_key` | API key for an OpenAI-compatible service. |
| `openai` | `model` | Model identifier, for example `gpt-4o-mini`. |
| `openai` | `base_url` | API base URL; defaults to `https://api.openai.com/v1`. |
| `db` | `url` | SQLite URL, for example `sqlite://corrode.db`. The database is created if it does not exist. |
| `db` | `students_csv` | Path to the student roster CSV; defaults to `students.csv`. |

You can omit `base_url` and `students_csv` if their defaults are suitable. The other values are required. See [`Config.example.toml`](Config.example.toml) for a complete template with comments.

### Assignment statements

Assignment statements are not included in this repository: `problems statements/` is excluded from Git. To include an assignment statement in reviews, create a file on the host at `problems statements/<assignment-id>_statement.md`. The assignment ID is the part of the repository name before the first hyphen. For example, `lab1-alice-smith` maps to `problems statements/lab1_statement.md`.

If the statement file is missing, Corrode continues the review using the diff and source files without assuming assignment requirements.

## Repository naming and student roster

Corrode maps repositories to students by GitHub login. A repository name must end with `-<login>`; the prefix before that suffix is treated as the assignment ID. Matching is case-insensitive.

```text
lab1-alice-smith
```

In this example, `lab1` is the assignment ID and `alice-smith` is the student's GitHub login. Repositories that do not match a student in the roster are not mapped to that student.

The CSV must have a header row and at least three columns in this order: first name, last name, and GitHub login. Rows with an empty value in any of these columns are skipped. A login may also be provided as `@login` or as a GitHub profile URL.

```csv
Name,Surname,GitHub
Alice,Smith,alice-smith
Bob,Johnson,@bobjohnson
```

The roster is imported into SQLite at startup. Re-importing updates the names of existing students; removing old rows from the CSV does not delete them from the database. See the [guide](docs/guide.md#student-roster) for more details.

## Using Corrode

After launch, navigate with `j`/`k` or the arrow keys and press `Enter` to choose an action:

| Menu item | Action |
| --- | --- |
| `Run all repositories once` | Poll the organization once, then return to the menu. |
| `Run all repositories continuously` | Poll continuously; press `q` or `Esc` to exit the mode. |
| `Run one student` | Select a student and run one polling iteration for their repositories. |
| `Database` | Show counts of students, assignments, repository mappings, and submissions. |
| `Settings` | Show current settings and reload the CSV by pressing `r`. |
| `Quit` | Exit the application. |

In the student list, press `r` to reload students from the database. `Esc` or `q` goes back or exits, depending on the screen; `Enter` selects an item or student. The interface displays the current operation status while a poll is running.

Logs are written to `corrode.log` in the current working directory. Set `RUST_LOG` to change the log level, for example:

```sh
RUST_LOG=debug cargo run --release
```

## Review workflow

1. Corrode fetches the repositories in the configured GitHub organization and their open PRs, processing API pages of up to 100 entries.
2. Draft PRs and commits that already have a published review are skipped.
3. Corrode clones the exact PR commit over SSH into a temporary directory and reads the changed text files. Individual files larger than 200 KB are skipped; source files are limited to 1 MB in total; binary files are ignored.
4. The model receives the PR title and description, diff, available source files, and the assignment statement if one was found. It returns JSON containing a summary, a decision, and inline comments.
5. Corrode publishes the review to GitHub and stores its status and result in SQLite. The temporary checkout is removed after processing.

The model is instructed to write comments in Russian, report only verifiable issues, and attach inline comments to changed lines. Processing errors are logged; a failed review can be retried during a later polling iteration.

## Data and privacy

- Corrode sends the diff, changed source files, PR title and description, and any available assignment statement to the LLM service configured by `openai.base_url`. Review the selected provider's data handling terms before use.
- `Config.toml` contains tokens and API keys, the CSV may contain students' personal data, and SQLite stores submission and review records. Do not commit these files; restrict access to them on the host.
- The GitHub token is used for GitHub API access and publishing reviews; the SSH key is used to clone repositories. Do not put secrets in source code or public logs.
- Repository source code is treated as untrusted model input. Corrode limits file sizes and checks relative paths when reading the checkout.

## Development

Run standard Cargo checks from the project root:

```sh
cargo fmt --check
cargo check
cargo test
```

To run the application manually, use `cargo run`; an interactive terminal and a populated `Config.toml` are required.

## Project layout

```text
src/
  main.rs      logging setup and application entry point
  tui.rs       terminal user interface
  app.rs       polling loop and PR processing
  github.rs    GitHub REST API and review submission
  reviewer.rs  OpenAI-compatible API client and model instructions
  runner.rs    commit checkout and source file reading
  db.rs        SQLite schema and queries for students, repositories, and reviews
  csv.rs       student roster import
  config.rs    TOML configuration loading
  models.rs    request and result data structures
  guide.md     detailed configuration and operations guide
```

## License

Corrode is distributed under the [MIT License](LICENSE).
