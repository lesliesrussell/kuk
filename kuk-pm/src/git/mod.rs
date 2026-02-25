use std::path::Path;

use crate::error::{PmError, Result};

/// Information about a git commit.
#[derive(Debug, Clone)]
pub struct CommitInfo {
    pub sha: String,
    pub message: String,
    pub author: String,
    pub time: i64,
}

/// Check if a path is inside a git repository.
pub fn is_git_repo(path: &Path) -> bool {
    gix::discover(path).is_ok()
}

/// Get the current branch name, or None if HEAD is detached.
pub fn current_branch(path: &Path) -> Result<Option<String>> {
    let repo = gix::discover(path).map_err(|e| PmError::Git(e.to_string()))?;
    match repo.head_ref().map_err(|e| PmError::Git(e.to_string()))? {
        Some(reference) => Ok(Some(reference.name().shorten().to_string())),
        None => Ok(None),
    }
}

/// Create a new branch pointing at HEAD.
pub fn create_branch(path: &Path, name: &str) -> Result<()> {
    let repo = gix::discover(path).map_err(|e| PmError::Git(e.to_string()))?;
    let head = repo
        .head_commit()
        .map_err(|e| PmError::Git(e.to_string()))?;
    let ref_name = format!("refs/heads/{name}");

    // Check if branch already exists
    if repo.find_reference(&ref_name).is_ok() {
        return Err(PmError::Git(format!("Branch already exists: {name}")));
    }

    repo.reference(
        ref_name,
        head.id,
        gix::refs::transaction::PreviousValue::MustNotExist,
        format!("kuk-pm: create branch {name}"),
    )
    .map_err(|e| PmError::Git(e.to_string()))?;
    Ok(())
}

/// Get the N most recent commits from HEAD.
pub fn recent_commits(path: &Path, count: usize) -> Result<Vec<CommitInfo>> {
    let repo = gix::discover(path).map_err(|e| PmError::Git(e.to_string()))?;
    let head = repo
        .head_commit()
        .map_err(|e| PmError::Git(e.to_string()))?;

    let mut commits = Vec::new();

    for ancestor in head
        .ancestors()
        .all()
        .map_err(|e| PmError::Git(e.to_string()))?
        .take(count)
    {
        let info = ancestor.map_err(|e| PmError::Git(e.to_string()))?;
        let commit = info.object().map_err(|e| PmError::Git(e.to_string()))?;

        commits.push(CommitInfo {
            sha: info.id.to_string(),
            message: commit.message_raw_sloppy().to_string(),
            author: commit
                .author()
                .map(|a| a.name.to_string())
                .unwrap_or_default(),
            time: commit.time().map(|t| t.seconds).unwrap_or(0),
        });
    }

    Ok(commits)
}

/// List all tag names in the repository.
pub fn list_tags(path: &Path) -> Result<Vec<String>> {
    let repo = gix::discover(path).map_err(|e| PmError::Git(e.to_string()))?;
    let references = repo.references().map_err(|e| PmError::Git(e.to_string()))?;
    let tag_refs = references
        .prefixed("refs/tags/")
        .map_err(|e| PmError::Git(e.to_string()))?;

    let mut tags = Vec::new();
    for reference in tag_refs {
        let reference = reference.map_err(|e| PmError::Git(e.to_string()))?;
        tags.push(reference.name().shorten().to_string());
    }

    Ok(tags)
}

/// Get commits between HEAD and a named ref (tag or branch).
/// Walks ancestors of HEAD and stops when reaching the target ref's commit.
pub fn commits_since_ref(path: &Path, ref_name: &str) -> Result<Vec<CommitInfo>> {
    let repo = gix::discover(path).map_err(|e| PmError::Git(e.to_string()))?;

    // Try multiple ref formats
    let target_id = repo
        .find_reference(&format!("refs/tags/{ref_name}"))
        .or_else(|_| repo.find_reference(&format!("refs/heads/{ref_name}")))
        .or_else(|_| repo.find_reference(ref_name))
        .map_err(|e| PmError::Git(format!("ref not found '{ref_name}': {e}")))?
        .id()
        .detach();

    let head = repo
        .head_commit()
        .map_err(|e| PmError::Git(e.to_string()))?;

    let mut commits = Vec::new();
    for ancestor in head
        .ancestors()
        .all()
        .map_err(|e| PmError::Git(e.to_string()))?
    {
        let info = ancestor.map_err(|e| PmError::Git(e.to_string()))?;
        if info.id == target_id {
            break;
        }
        let commit = info.object().map_err(|e| PmError::Git(e.to_string()))?;
        commits.push(CommitInfo {
            sha: info.id.to_string(),
            message: commit.message_raw_sloppy().to_string(),
            author: commit
                .author()
                .map(|a| a.name.to_string())
                .unwrap_or_default(),
            time: commit.time().map(|t| t.seconds).unwrap_or(0),
        });
    }

    Ok(commits)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;
    use tempfile::TempDir;

    fn init_git_repo() -> TempDir {
        let dir = TempDir::new().unwrap();
        Command::new("git")
            .args(["init"])
            .current_dir(dir.path())
            .output()
            .unwrap();
        Command::new("git")
            .args(["config", "user.email", "test@test.com"])
            .current_dir(dir.path())
            .output()
            .unwrap();
        Command::new("git")
            .args(["config", "user.name", "Test"])
            .current_dir(dir.path())
            .output()
            .unwrap();
        // Create initial commit
        std::fs::write(dir.path().join("README.md"), "# Test").unwrap();
        Command::new("git")
            .args(["add", "."])
            .current_dir(dir.path())
            .output()
            .unwrap();
        Command::new("git")
            .args(["commit", "-m", "Initial commit"])
            .current_dir(dir.path())
            .output()
            .unwrap();
        dir
    }

    #[test]
    fn detect_git_repo() {
        let dir = init_git_repo();
        assert!(is_git_repo(dir.path()));
    }

    #[test]
    fn detect_non_git_repo() {
        let dir = TempDir::new().unwrap();
        assert!(!is_git_repo(dir.path()));
    }

    #[test]
    fn get_current_branch() {
        let dir = init_git_repo();
        let branch = current_branch(dir.path()).unwrap();
        assert!(branch.is_some());
        let name = branch.unwrap();
        assert!(
            name == "main" || name == "master",
            "Unexpected branch: {name}"
        );
    }

    #[test]
    fn create_and_verify_branch() {
        let dir = init_git_repo();
        create_branch(dir.path(), "feature/test-branch").unwrap();

        // Verify branch exists via git CLI
        let output = Command::new("git")
            .args(["branch", "--list", "feature/test-branch"])
            .current_dir(dir.path())
            .output()
            .unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("feature/test-branch"));
    }

    #[test]
    fn create_duplicate_branch_fails() {
        let dir = init_git_repo();
        create_branch(dir.path(), "feature/dup").unwrap();
        let result = create_branch(dir.path(), "feature/dup");
        assert!(result.is_err());
    }

    #[test]
    fn recent_commits_returns_history() {
        let dir = init_git_repo();
        // Add a second commit
        std::fs::write(dir.path().join("file.txt"), "content").unwrap();
        Command::new("git")
            .args(["add", "."])
            .current_dir(dir.path())
            .output()
            .unwrap();
        Command::new("git")
            .args(["commit", "-m", "Second commit"])
            .current_dir(dir.path())
            .output()
            .unwrap();

        let commits = recent_commits(dir.path(), 10).unwrap();
        assert_eq!(commits.len(), 2);
        assert!(commits[0].message.contains("Second commit"));
        assert!(commits[1].message.contains("Initial commit"));
    }

    #[test]
    fn recent_commits_limited() {
        let dir = init_git_repo();
        std::fs::write(dir.path().join("file.txt"), "content").unwrap();
        Command::new("git")
            .args(["add", "."])
            .current_dir(dir.path())
            .output()
            .unwrap();
        Command::new("git")
            .args(["commit", "-m", "Second"])
            .current_dir(dir.path())
            .output()
            .unwrap();

        let commits = recent_commits(dir.path(), 1).unwrap();
        assert_eq!(commits.len(), 1);
    }

    #[test]
    fn recent_commits_has_author() {
        let dir = init_git_repo();
        let commits = recent_commits(dir.path(), 1).unwrap();
        assert_eq!(commits.len(), 1);
        assert_eq!(commits[0].author, "Test");
    }
}
