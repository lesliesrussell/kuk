use std::path::Path;
use std::process::Command;

use serde::Serialize;

use kuk::model::Card;
use kuk::storage::Store;

use crate::error::{PmError, Result};
use crate::model::GitMetadata;

// ─── Types ───────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct SyncAction {
    pub card_title: String,
    pub card_id: String,
    pub action: SyncActionType,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SyncActionType {
    UpdateColumn,
    UpdateUrl,
    Skip,
}

// ─── Sync logic ──────────────────────────────────────────────

/// Run bidirectional sync. Returns list of actions taken (or that would be
/// taken if dry_run is true).
pub fn run_sync(repo: &Path, dry_run: bool, json_output: bool) -> Result<Vec<SyncAction>> {
    let store = Store::new(repo);
    if !store.is_initialized() {
        return Err(PmError::KukNotInitialized);
    }

    // Check gh CLI availability
    if !is_gh_available() {
        return Err(PmError::Other(
            "GitHub CLI (gh) not found. Install it from https://cli.github.com/".into(),
        ));
    }

    let config = store.load_config()?;
    let mut board = store.load_board(&config.default_board)?;

    let mut actions = Vec::new();

    for card in &mut board.cards {
        if card.archived {
            continue;
        }

        let meta = get_pm_metadata(card);

        // Check linked issues
        if let Some(ref issue_url) = meta.issue_url {
            match fetch_issue_state(issue_url) {
                Ok(state) => {
                    let target_column = match state.as_str() {
                        "closed" => Some("done"),
                        "open" => None, // don't move open issues
                        _ => None,
                    };

                    if let Some(col) = target_column
                        && card.column != col
                    {
                        actions.push(SyncAction {
                            card_title: card.title.clone(),
                            card_id: card.id.clone(),
                            action: SyncActionType::UpdateColumn,
                            detail: format!("{} → {col} (issue {state})", card.column),
                        });
                        if !dry_run {
                            card.column = col.to_string();
                            card.updated_at = chrono::Utc::now();
                        }
                    }
                }
                Err(e) => {
                    actions.push(SyncAction {
                        card_title: card.title.clone(),
                        card_id: card.id.clone(),
                        action: SyncActionType::Skip,
                        detail: format!("failed to fetch issue: {e}"),
                    });
                }
            }
        }

        // Check linked PRs
        if let Some(ref pr_url) = meta.pr_url {
            match fetch_pr_state(pr_url) {
                Ok(state) => {
                    let target_column = match state.as_str() {
                        "merged" | "closed" => Some("done"),
                        "open" => None,
                        _ => None,
                    };

                    if let Some(col) = target_column
                        && card.column != col
                    {
                        actions.push(SyncAction {
                            card_title: card.title.clone(),
                            card_id: card.id.clone(),
                            action: SyncActionType::UpdateColumn,
                            detail: format!("{} → {col} (PR {state})", card.column),
                        });
                        if !dry_run {
                            card.column = col.to_string();
                            card.updated_at = chrono::Utc::now();
                        }
                    }
                }
                Err(e) => {
                    actions.push(SyncAction {
                        card_title: card.title.clone(),
                        card_id: card.id.clone(),
                        action: SyncActionType::Skip,
                        detail: format!("failed to fetch PR: {e}"),
                    });
                }
            }
        }
    }

    if !dry_run
        && actions
            .iter()
            .any(|a| matches!(a.action, SyncActionType::UpdateColumn))
    {
        store.save_board(&board)?;
    }

    // Output
    if json_output {
        println!("{}", serde_json::to_string_pretty(&actions)?);
    } else if actions.is_empty() {
        println!("Everything up to date.");
    } else {
        if dry_run {
            println!("Dry run — no changes applied:\n");
        }
        for action in &actions {
            let prefix = match action.action {
                SyncActionType::UpdateColumn => "  [SYNC]",
                SyncActionType::UpdateUrl => "  [LINK]",
                SyncActionType::Skip => "  [SKIP]",
            };
            println!("{prefix} {} — {}", action.card_title, action.detail);
        }
        println!(
            "\n{} action(s){}",
            actions.len(),
            if dry_run { " (dry run)" } else { " applied" }
        );
    }

    Ok(actions)
}

// ─── GitHub API helpers ──────────────────────────────────────

fn is_gh_available() -> bool {
    Command::new("gh")
        .arg("--version")
        .output()
        .is_ok_and(|o| o.status.success())
}

/// Parse a GitHub issue/PR URL into (owner, repo, number).
fn parse_github_url(url: &str) -> Option<(String, String, String)> {
    // https://github.com/owner/repo/issues/42
    // https://github.com/owner/repo/pull/42
    let parts: Vec<&str> = url.trim_end_matches('/').split('/').collect();
    if parts.len() >= 5 {
        let owner = parts[parts.len() - 4].to_string();
        let repo = parts[parts.len() - 3].to_string();
        let number = parts[parts.len() - 1].to_string();
        Some((owner, repo, number))
    } else {
        None
    }
}

fn fetch_issue_state(url: &str) -> Result<String> {
    let (owner, repo, number) =
        parse_github_url(url).ok_or_else(|| PmError::Other(format!("invalid URL: {url}")))?;

    let output = Command::new("gh")
        .args([
            "api",
            &format!("repos/{owner}/{repo}/issues/{number}"),
            "--jq",
            ".state",
        ])
        .output()
        .map_err(|e| PmError::Other(format!("gh api failed: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(PmError::Other(format!("gh api error: {stderr}")));
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn fetch_pr_state(url: &str) -> Result<String> {
    let (owner, repo, number) =
        parse_github_url(url).ok_or_else(|| PmError::Other(format!("invalid URL: {url}")))?;

    let output = Command::new("gh")
        .args([
            "api",
            &format!("repos/{owner}/{repo}/pulls/{number}"),
            "--jq",
            "if .merged then \"merged\" elif .state == \"closed\" then \"closed\" else .state end",
        ])
        .output()
        .map_err(|e| PmError::Other(format!("gh api failed: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(PmError::Other(format!("gh api error: {stderr}")));
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

// ─── PR creation ─────────────────────────────────────────────

/// Create a GitHub PR from the current branch. Returns the PR URL.
pub fn create_pr(repo: &Path, title: &str, body: &str) -> Result<String> {
    if !is_gh_available() {
        return Err(PmError::Other(
            "GitHub CLI (gh) not found. Install it from https://cli.github.com/".into(),
        ));
    }

    let output = Command::new("gh")
        .args(["pr", "create", "--title", title, "--body", body])
        .current_dir(repo)
        .output()
        .map_err(|e| PmError::Other(format!("gh pr create failed: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(PmError::Other(format!("gh pr create error: {stderr}")));
    }

    let pr_url = String::from_utf8_lossy(&output.stdout).trim().to_string();
    Ok(pr_url)
}

// ─── Card metadata helpers ───────────────────────────────────

pub fn get_pm_metadata(card: &Card) -> GitMetadata {
    card.metadata
        .get("pm")
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default()
}

pub fn set_pm_metadata(card: &mut Card, meta: &GitMetadata) {
    if let Ok(value) = serde_json::to_value(meta) {
        card.metadata.insert("pm".into(), value);
    }
}

// ─── Tests ───────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_github_issue_url() {
        let (owner, repo, number) =
            parse_github_url("https://github.com/user/myrepo/issues/42").unwrap();
        assert_eq!(owner, "user");
        assert_eq!(repo, "myrepo");
        assert_eq!(number, "42");
    }

    #[test]
    fn parse_github_pr_url() {
        let (owner, repo, number) =
            parse_github_url("https://github.com/org/project/pull/7").unwrap();
        assert_eq!(owner, "org");
        assert_eq!(repo, "project");
        assert_eq!(number, "7");
    }

    #[test]
    fn parse_invalid_url() {
        assert!(parse_github_url("not-a-url").is_none());
        assert!(parse_github_url("https://github.com/user").is_none());
    }

    #[test]
    fn parse_trailing_slash() {
        let (_, _, number) = parse_github_url("https://github.com/user/repo/issues/99/").unwrap();
        assert_eq!(number, "99");
    }

    #[test]
    fn pm_metadata_roundtrip_on_card() {
        let mut card = Card::new("Test", "todo");
        let meta = GitMetadata {
            branch: Some("feature/test".into()),
            issue_url: Some("https://github.com/u/r/issues/1".into()),
            ..Default::default()
        };
        set_pm_metadata(&mut card, &meta);

        let loaded = get_pm_metadata(&card);
        assert_eq!(loaded.branch.unwrap(), "feature/test");
        assert!(loaded.issue_url.is_some());
    }

    #[test]
    fn pm_metadata_default_on_clean_card() {
        let card = Card::new("Clean", "todo");
        let meta = get_pm_metadata(&card);
        assert!(meta.branch.is_none());
        assert!(meta.issue_url.is_none());
        assert!(meta.pr_url.is_none());
    }
}
