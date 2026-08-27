//! Portfolio-mode project discovery.
//!
//! A "project" is a directory that contains a `goglz.yaml` file. The
//! `discover_projects` helper walks the user's home directory (or any other
//! starting point) and returns all such project roots, suitable for batch
//! operations like `goglz --portfolio revise`.

use crate::error::Result;
use std::path::{Path, PathBuf};
use walkdir::{DirEntry, WalkDir};

/// Default maximum directory depth when scanning for projects.
pub const DEFAULT_MAX_DEPTH: usize = 4;

/// Name of the file that marks a directory as a goglz-managed project.
pub const PROJECT_MARKER: &str = "goglz.yaml";

/// True for entries whose file name starts with a dot. Used to prune hidden
/// directories (and skip hidden files) from the project walk.
fn is_hidden(entry: &DirEntry) -> bool {
    entry.depth() > 0
        && entry
            .file_name()
            .to_str()
            .map(|s| s.starts_with('.'))
            .unwrap_or(false)
}

/// Returns true if `path` is a project root (i.e. it contains `goglz.yaml`).
pub fn is_project_root(path: &Path) -> bool {
    path.join(PROJECT_MARKER).is_file()
}

/// Discover goglz-managed projects under `home` up to `max_depth` levels deep.
///
/// Hidden directories and files are ignored. Results are sorted so that output
/// and processing order are deterministic.
pub fn discover_projects(home: &Path, max_depth: usize) -> Result<Vec<PathBuf>> {
    let mut projects = Vec::new();

    for entry in WalkDir::new(home)
        .max_depth(max_depth)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| !is_hidden(e))
        .filter_map(|e| e.ok())
    {
        let path = entry.path();

        if path.is_dir() && is_project_root(path) {
            projects.push(path.to_path_buf());
        }
    }

    projects.sort();
    Ok(projects)
}

/// Build the default set of monitored-directory patterns used when the daemon
/// is started in portfolio mode.
pub fn default_portfolio_patterns() -> Vec<String> {
    vec!["*.md".to_string(), "*.txt".to_string(), "*.rst".to_string()]
}

/// Convenience wrapper that discovers projects under `home` using
/// [`DEFAULT_MAX_DEPTH`].
pub fn discover_projects_default(home: &Path) -> Result<Vec<PathBuf>> {
    discover_projects(home, DEFAULT_MAX_DEPTH)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn is_project_root_detects_goglz_yaml() {
        let dir = tempfile::tempdir().unwrap();
        assert!(!is_project_root(dir.path()));
        fs::write(dir.path().join(PROJECT_MARKER), "purpose: test\n").unwrap();
        assert!(is_project_root(dir.path()));
    }

    #[test]
    fn discover_projects_finds_only_marked_dirs() {
        let home = tempfile::tempdir().unwrap();
        fs::create_dir_all(home.path().join("project-a/docs")).unwrap();
        fs::create_dir_all(home.path().join("project-b")).unwrap();
        fs::create_dir_all(home.path().join("not-a-project")).unwrap();

        fs::write(home.path().join("project-a").join(PROJECT_MARKER), "purpose: a\n").unwrap();
        fs::write(home.path().join("project-b").join(PROJECT_MARKER), "purpose: b\n").unwrap();

        let projects = discover_projects_default(home.path()).unwrap();
        assert_eq!(projects.len(), 2);
        assert!(projects[0].ends_with("project-a"));
        assert!(projects[1].ends_with("project-b"));
    }

    #[test]
    fn discover_projects_skips_hidden_directories() {
        let home = tempfile::tempdir().unwrap();
        fs::create_dir_all(home.path().join(".hidden/project")).unwrap();
        fs::write(
            home.path().join(".hidden/project").join(PROJECT_MARKER),
            "purpose: hidden\n",
        )
        .unwrap();

        let projects = discover_projects_default(home.path()).unwrap();
        assert!(projects.is_empty());
    }

    #[test]
    fn discover_projects_respects_max_depth() {
        let home = tempfile::tempdir().unwrap();
        fs::create_dir_all(home.path().join("a/b/c/d/e/project")).unwrap();
        fs::write(
            home.path().join("a/b/c/d/e/project").join(PROJECT_MARKER),
            "purpose: deep\n",
        )
        .unwrap();

        // DEFAULT_MAX_DEPTH is 4, so the project at depth 6 should not appear.
        let projects = discover_projects_default(home.path()).unwrap();
        assert!(projects.is_empty());

        // With a larger depth it should be found.
        let projects = discover_projects(home.path(), 8).unwrap();
        assert_eq!(projects.len(), 1);
    }
}
