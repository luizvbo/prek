use crate::cli::ExitStatus;
use crate::config::{Config, Repo};
use crate::printer::Printer;
use crate::store::STORE;
use anyhow::Result;
use owo_colors::OwoColorize;
use std::collections::HashSet;
use std::fmt::Write;
use std::path::PathBuf;
use tracing::debug;

pub(crate) fn gc(printer: Printer) -> Result<ExitStatus> {
    let store = STORE.as_ref()?;
    let _lock = store.lock()?;

    // Get all cloned repos from the store, handling non-existent directory
    let all_repos_on_disk = if store.repos_dir().exists() {
        store
            .repos_dir()
            .read_dir()?
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .collect::<HashSet<_>>()
    } else {
        HashSet::new()
    };

    // Get all "used" configs and filter out those that no longer exist
    let used_configs = store.select_all_configs()?;
    let live_configs: Vec<Config> = used_configs
        .into_iter()
        .filter_map(|path| crate::config::read_config(&path).ok())
        .collect();

    // Determine the set of all repos that are actually in use
    let mut used_repo_paths = HashSet::new();
    for config in &live_configs {
        for repo in &config.repos {
            if let Repo::Remote(remote_repo) = repo {
                let repo_path = store.repo_path(remote_repo);
                used_repo_paths.insert(repo_path);
            }
        }
    }

    // Determine which repos are unused
    let unused_repos: Vec<PathBuf> = all_repos_on_disk
        .difference(&used_repo_paths)
        .cloned()
        .collect();

    // Delete unused repos
    for repo_path in &unused_repos {
        debug!("Removing unused repo: {}", repo_path.display());
        // Use a synchronous delete
        store.delete_repo(repo_path)?;
    }

    writeln!(
        printer.stdout(),
        "{} repo(s) removed.",
        unused_repos.len().cyan()
    )?;

    Ok(ExitStatus::Success)
}
