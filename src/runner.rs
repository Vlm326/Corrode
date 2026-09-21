use std::path::{Component, Path, PathBuf};

use thiserror::Error;
use tokio::process::Command;

use super::models::PullRequestFile;

const MAX_FILE_BYTES: u64 = 200_000;
const MAX_TOTAL_SOURCE_BYTES: u64 = 1_000_000;

#[derive(Debug, Error)]
pub enum RunnerError {
    #[error("git command failed: {0}")]
    Git(String),
    #[error("failed to read source file {path}: {source}")]
    Read {
        path: String,
        source: std::io::Error,
    },
}

#[derive(Debug, Clone)]
pub struct SourceFile {
    pub path: String,
    pub content: String,
}

pub async fn clone_and_read_sources(
    token: &str,
    repository: &str,
    commit_sha: &str,
    files: &[PullRequestFile],
) -> Result<Vec<SourceFile>, RunnerError> {
    let worktree = temporary_worktree(repository, commit_sha);
    if worktree.exists() {
        tokio::fs::remove_dir_all(&worktree)
            .await
            .map_err(|error| RunnerError::Git(error.to_string()))?;
    }

    let result = clone_commit(token, repository, commit_sha, &worktree).await;
    let sources = match result {
        Ok(()) => read_changed_sources(&worktree, files).await,
        Err(error) => Err(error),
    };

    let _ = tokio::fs::remove_dir_all(&worktree).await;
    sources
}

async fn clone_commit(
    token: &str,
    repository: &str,
    commit_sha: &str,
    worktree: &Path,
) -> Result<(), RunnerError> {
    let url = format!("https://github.com/{repository}.git");
    let clone = git_command(token)
        .args([
            "clone",
            "--no-checkout",
            "--filter=blob:none",
            "--depth=1",
            &url,
            worktree.to_string_lossy().as_ref(),
        ])
        .output()
        .await
        .map_err(|error| RunnerError::Git(error.to_string()))?;
    if !clone.status.success() {
        return Err(RunnerError::Git(command_output(&clone.stderr)));
    }

    let fetch = git_command(token)
        .args([
            "-C",
            worktree.to_string_lossy().as_ref(),
            "fetch",
            "--depth=1",
            "origin",
            commit_sha,
        ])
        .output()
        .await
        .map_err(|error| RunnerError::Git(error.to_string()))?;
    if !fetch.status.success() {
        return Err(RunnerError::Git(command_output(&fetch.stderr)));
    }

    let checkout = git_command(token)
        .args([
            "-C",
            worktree.to_string_lossy().as_ref(),
            "checkout",
            "--detach",
            commit_sha,
        ])
        .output()
        .await
        .map_err(|error| RunnerError::Git(error.to_string()))?;
    if !checkout.status.success() {
        return Err(RunnerError::Git(command_output(&checkout.stderr)));
    }

    Ok(())
}

fn git_command(token: &str) -> Command {
    let mut command = Command::new("git");
    command.env("GIT_CONFIG_COUNT", "1");
    command.env("GIT_CONFIG_KEY_0", "http.extraheader");
    command.env(
        "GIT_CONFIG_VALUE_0",
        format!("AUTHORIZATION: bearer {token}"),
    );
    command
}

async fn read_changed_sources(
    worktree: &Path,
    files: &[PullRequestFile],
) -> Result<Vec<SourceFile>, RunnerError> {
    let mut sources = Vec::new();
    let mut total_bytes = 0;
    for file in files {
        let relative_path = Path::new(&file.filename);
        if !is_safe_relative_path(relative_path) {
            continue;
        }
        let path = worktree.join(relative_path);
        let metadata = match tokio::fs::metadata(&path).await {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(source) => {
                return Err(RunnerError::Read {
                    path: file.filename.clone(),
                    source,
                });
            }
        };
        if metadata.len() > MAX_FILE_BYTES {
            continue;
        }
        if total_bytes + metadata.len() > MAX_TOTAL_SOURCE_BYTES {
            break;
        }
        let content = match tokio::fs::read_to_string(&path).await {
            Ok(content) => content,
            Err(source) if source.kind() == std::io::ErrorKind::InvalidData => continue,
            Err(source) => {
                return Err(RunnerError::Read {
                    path: file.filename.clone(),
                    source,
                });
            }
        };
        total_bytes += metadata.len();
        sources.push(SourceFile {
            path: file.filename.clone(),
            content,
        });
    }
    Ok(sources)
}

fn is_safe_relative_path(path: &Path) -> bool {
    !path.is_absolute()
        && path
            .components()
            .all(|component| !matches!(component, Component::ParentDir))
}

fn temporary_worktree(repository: &str, commit_sha: &str) -> PathBuf {
    let repository = repository.replace('/', "-");
    let commit = &commit_sha[..commit_sha.len().min(12)];
    std::env::temp_dir().join(format!("corrode-{repository}-{commit}"))
}

fn command_output(output: &[u8]) -> String {
    String::from_utf8_lossy(output).trim().to_string()
}
