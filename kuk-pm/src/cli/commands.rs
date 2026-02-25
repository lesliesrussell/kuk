use std::path::{Path, PathBuf};

use chrono::NaiveDate;
use clap::{Parser, Subcommand};

use crate::error::{PmError, Result};
use crate::git;
use crate::model::{PmConfig, Sprint, SprintStatus};
use crate::reports;
use crate::sync;
use kuk::storage::Store;

#[derive(Parser, Debug)]
#[command(
    name = "kuk-pm",
    version,
    about = "Git-native project manager for kuk."
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,

    /// Output as JSON
    #[arg(long, global = true)]
    pub json: bool,

    /// Suppress non-essential output
    #[arg(long, global = true)]
    pub quiet: bool,

    /// Path to repo root (defaults to current directory)
    #[arg(long, global = true)]
    pub repo: Option<PathBuf>,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Initialize kuk-pm in the current repo
    Init,

    /// Cross-repo project overview
    Projects,

    /// Bidirectional sync with GitHub/GitLab
    Sync {
        /// Preview changes without applying
        #[arg(long)]
        dry_run: bool,
    },

    /// Link a card to an issue or PR URL
    Link {
        /// Card ID or number
        card_id: String,
        /// Issue or PR URL
        url: String,
    },

    /// Create a git branch from a card
    Branch {
        /// Card ID or number
        card_id: String,
    },

    /// Create a PR from the current branch
    Pr {
        /// Card ID or number
        card_id: String,
    },

    /// Show velocity metrics
    Velocity {
        /// Number of weeks to analyze
        #[arg(long, default_value = "4")]
        weeks: u32,
        /// Target repo path (or "all")
        #[arg(long)]
        target: Option<String>,
    },

    /// Show burndown chart
    Burndown {
        /// Sprint name
        #[arg(long)]
        sprint: Option<String>,
    },

    /// Show roadmap
    Roadmap {
        /// Number of weeks to project
        #[arg(long, default_value = "12")]
        weeks: u32,
    },

    /// Generate release notes
    ReleaseNotes {
        /// Starting point (tag or ref)
        #[arg(long, default_value = "last-tag")]
        since: Option<String>,
    },

    /// Sprint management
    Sprint {
        #[command(subcommand)]
        command: SprintCmd,
    },

    /// Show project statistics
    Stats,

    /// Run as MCP server (stdio transport for Claude Code / AI agents)
    Mcp,

    /// Health check
    Doctor,

    /// Show version
    Version,
}

#[derive(Subcommand, Debug)]
pub enum SprintCmd {
    /// Create a new sprint
    Create {
        /// Sprint name
        name: String,
        /// Start date (YYYY-MM-DD)
        #[arg(long)]
        start: String,
        /// End date (YYYY-MM-DD)
        #[arg(long)]
        end: String,
    },
    /// Close an active sprint
    Close {
        /// Sprint name
        name: String,
    },
    /// List all sprints
    List,
}

// --- Command implementations ---

pub fn init(repo: &Path) -> Result<()> {
    let store = Store::new(repo);
    if !store.is_initialized() {
        return Err(PmError::KukNotInitialized);
    }

    let pm_config_path = store.kuk_dir().join("pm.json");
    if pm_config_path.exists() {
        return Err(PmError::AlreadyInitialized(
            store.kuk_dir().display().to_string(),
        ));
    }

    // Create pm.json
    let pm_config = PmConfig::default();
    let json = serde_json::to_string_pretty(&pm_config)?;
    std::fs::write(&pm_config_path, json)?;

    // Create sprints.json
    let sprints_path = store.kuk_dir().join("sprints.json");
    let sprints: Vec<Sprint> = Vec::new();
    let json = serde_json::to_string_pretty(&sprints)?;
    std::fs::write(&sprints_path, json)?;

    // Check git repo
    let git_status = if git::is_git_repo(repo) {
        "git repo detected"
    } else {
        "no git repo (git integration disabled)"
    };

    println!("Initialized kuk-pm in {}", store.kuk_dir().display());
    println!("  {git_status}");
    Ok(())
}

pub fn projects(json_output: bool) -> Result<()> {
    let index = Store::load_global_index().unwrap_or_default();

    if json_output {
        println!("{}", serde_json::to_string_pretty(&index)?);
        return Ok(());
    }

    if index.projects.is_empty() {
        println!("No kuk projects found. Run `kuk init` in a repo.");
        return Ok(());
    }

    for p in &index.projects {
        let git_info = if git::is_git_repo(Path::new(&p.path)) {
            match git::current_branch(Path::new(&p.path)) {
                Ok(Some(branch)) => format!(" ({branch})"),
                _ => " (git)".into(),
            }
        } else {
            String::new()
        };
        println!("  {} → {}{}", p.name, p.path, git_info);
    }
    Ok(())
}

pub fn branch(repo: &Path, card_id: &str, json_output: bool) -> Result<()> {
    let store = Store::new(repo);
    if !store.is_initialized() {
        return Err(PmError::KukNotInitialized);
    }

    if !git::is_git_repo(repo) {
        return Err(PmError::NotGitRepo);
    }

    let config = store.load_config()?;
    let board = store.load_board(&config.default_board)?;

    let card_uuid = board
        .resolve_card_id(card_id)
        .ok_or_else(|| PmError::CardNotFound(card_id.into()))?;

    let card = board
        .find_card(&card_uuid)
        .ok_or_else(|| PmError::CardNotFound(card_id.into()))?;

    let branch_name = slugify_branch(&card.title);
    git::create_branch(repo, &branch_name)?;

    if json_output {
        println!(
            "{}",
            serde_json::json!({
                "card_id": card_uuid,
                "branch": branch_name,
                "title": card.title
            })
        );
    } else {
        println!(
            "Created branch: {} (from card: {})",
            branch_name, card.title
        );
    }
    Ok(())
}

pub fn doctor(repo: &Path) -> Result<()> {
    println!("kuk-pm doctor");
    println!("─────────────");

    // Check kuk init
    let store = Store::new(repo);
    if store.is_initialized() {
        println!("  [OK] .kuk/ directory found");
    } else {
        println!("  [!!] .kuk/ not found. Run `kuk init` first.");
        return Ok(());
    }

    // Check kuk config
    match store.load_config() {
        Ok(config) => println!(
            "  [OK] kuk config (v{}, board: {})",
            config.version, config.default_board
        ),
        Err(e) => println!("  [!!] kuk config: {e}"),
    }

    // Check pm.json
    let pm_path = store.kuk_dir().join("pm.json");
    if pm_path.exists() {
        match std::fs::read_to_string(&pm_path) {
            Ok(data) => match serde_json::from_str::<PmConfig>(&data) {
                Ok(_) => println!("  [OK] pm.json"),
                Err(e) => println!("  [!!] pm.json parse error: {e}"),
            },
            Err(e) => println!("  [!!] pm.json read error: {e}"),
        }
    } else {
        println!("  [--] pm.json not found (run `kuk-pm init`)");
    }

    // Check sprints.json
    let sprints_path = store.kuk_dir().join("sprints.json");
    if sprints_path.exists() {
        match std::fs::read_to_string(&sprints_path) {
            Ok(data) => match serde_json::from_str::<Vec<Sprint>>(&data) {
                Ok(sprints) => println!("  [OK] sprints.json ({} sprints)", sprints.len()),
                Err(e) => println!("  [!!] sprints.json parse error: {e}"),
            },
            Err(e) => println!("  [!!] sprints.json read error: {e}"),
        }
    } else {
        println!("  [--] sprints.json not found (run `kuk-pm init`)");
    }

    // Check git
    if git::is_git_repo(repo) {
        println!("  [OK] git repository detected");
        match git::current_branch(repo) {
            Ok(Some(branch)) => println!("       └─ branch: {branch}"),
            Ok(None) => println!("       └─ detached HEAD"),
            Err(e) => println!("       └─ error reading branch: {e}"),
        }
    } else {
        println!("  [--] not a git repository (git features disabled)");
    }

    // Check boards
    match store.list_boards() {
        Ok(boards) => {
            println!("  [OK] {} board(s): {}", boards.len(), boards.join(", "));
            for b in &boards {
                match store.load_board(b) {
                    Ok(board) => {
                        let active = board.cards.iter().filter(|c| !c.archived).count();
                        let archived = board.cards.iter().filter(|c| c.archived).count();
                        println!("       └─ {b}: {active} active, {archived} archived");
                    }
                    Err(e) => println!("       └─ {b}: ERROR: {e}"),
                }
            }
        }
        Err(e) => println!("  [!!] boards: {e}"),
    }

    // Check global index
    match Store::load_global_index() {
        Some(index) => println!("  [OK] global index: {} projects", index.projects.len()),
        None => println!("  [--] global index: not found (optional)"),
    }

    println!("\nAll checks passed.");
    Ok(())
}

pub fn version() -> Result<()> {
    println!("kuk-pm {}", env!("CARGO_PKG_VERSION"));
    Ok(())
}

pub fn default_action() -> Result<()> {
    println!("kuk-pm — Git-native project manager for kuk.");
    println!();
    println!("Run `kuk-pm --help` for usage or `kuk-pm init` to get started.");
    Ok(())
}

// --- Helpers ---

fn slugify_branch(title: &str) -> String {
    let slug: String = title
        .to_lowercase()
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' {
                c
            } else {
                '-'
            }
        })
        .collect();

    // Collapse multiple dashes and trim
    let mut result = String::new();
    let mut last_dash = false;
    for c in slug.trim_matches('-').chars() {
        if c == '-' {
            if !last_dash {
                result.push(c);
                last_dash = true;
            }
        } else {
            result.push(c);
            last_dash = false;
        }
    }
    format!("feature/{result}")
}

// ─── Sprint CRUD ─────────────────────────────────────────────

pub fn sprint(repo: &Path, command: SprintCmd, json_output: bool) -> Result<()> {
    let store = Store::new(repo);
    if !store.is_initialized() {
        return Err(PmError::KukNotInitialized);
    }

    match command {
        SprintCmd::Create { name, start, end } => {
            sprint_create(&store, &name, &start, &end, json_output)
        }
        SprintCmd::Close { name } => sprint_close(&store, &name, json_output),
        SprintCmd::List => sprint_list(&store, json_output),
    }
}

fn sprint_create(
    store: &Store,
    name: &str,
    start_str: &str,
    end_str: &str,
    json_output: bool,
) -> Result<()> {
    let start = NaiveDate::parse_from_str(start_str, "%Y-%m-%d")
        .map_err(|_| PmError::InvalidDate(format!("invalid start date: {start_str}")))?;
    let end = NaiveDate::parse_from_str(end_str, "%Y-%m-%d")
        .map_err(|_| PmError::InvalidDate(format!("invalid end date: {end_str}")))?;

    if end <= start {
        return Err(PmError::InvalidDate(
            "end date must be after start date".into(),
        ));
    }

    let mut sprints = load_sprints(store)?;

    if sprints.iter().any(|s| s.name == name) {
        return Err(PmError::SprintAlreadyExists(name.into()));
    }

    let sprint = Sprint {
        name: name.into(),
        start,
        end,
        goal: None,
        boards: Vec::new(),
        status: SprintStatus::Planned,
    };

    sprints.push(sprint.clone());
    save_sprints(store, &sprints)?;

    if json_output {
        println!("{}", serde_json::to_string_pretty(&sprint)?);
    } else {
        println!("Created sprint: {name} ({start} → {end})");
    }
    Ok(())
}

fn sprint_close(store: &Store, name: &str, json_output: bool) -> Result<()> {
    let mut sprints = load_sprints(store)?;

    let sprint = sprints
        .iter_mut()
        .find(|s| s.name == name)
        .ok_or_else(|| PmError::SprintNotFound(name.into()))?;

    if sprint.status == SprintStatus::Closed {
        return Err(PmError::SprintAlreadyClosed(name.into()));
    }

    sprint.status = SprintStatus::Closed;
    let result = sprint.clone();

    save_sprints(store, &sprints)?;

    if json_output {
        println!("{}", serde_json::to_string_pretty(&result)?);
    } else {
        println!("Closed sprint: {name}");
    }
    Ok(())
}

fn sprint_list(store: &Store, json_output: bool) -> Result<()> {
    let sprints = load_sprints(store)?;

    if json_output {
        println!("{}", serde_json::to_string_pretty(&sprints)?);
        return Ok(());
    }

    if sprints.is_empty() {
        println!("No sprints defined. Create one with `kuk-pm sprint create`.");
        return Ok(());
    }

    println!("Sprints");
    println!("───────");
    for s in &sprints {
        let status = match s.status {
            SprintStatus::Planned => "planned",
            SprintStatus::Active => "active",
            SprintStatus::Closed => "closed",
        };
        println!("  {} ({} → {}) [{}]", s.name, s.start, s.end, status);
    }
    Ok(())
}

// ─── Link ────────────────────────────────────────────────────

pub fn link(repo: &Path, card_id: &str, url: &str, json_output: bool) -> Result<()> {
    let store = Store::new(repo);
    if !store.is_initialized() {
        return Err(PmError::KukNotInitialized);
    }

    let config = store.load_config()?;
    let mut board = store.load_board(&config.default_board)?;

    let card_uuid = board
        .resolve_card_id(card_id)
        .ok_or_else(|| PmError::CardNotFound(card_id.into()))?;

    let card = board
        .find_card_mut(&card_uuid)
        .ok_or_else(|| PmError::CardNotFound(card_id.into()))?;

    let mut meta = sync::get_pm_metadata(card);

    // Detect issue vs PR from URL
    if url.contains("/pull/") || url.contains("/pulls/") || url.contains("/merge_requests/") {
        meta.pr_url = Some(url.into());
    } else {
        meta.issue_url = Some(url.into());
    }

    sync::set_pm_metadata(card, &meta);
    card.updated_at = chrono::Utc::now();

    store.save_board(&board)?;

    if json_output {
        println!(
            "{}",
            serde_json::json!({
                "card_id": card_uuid,
                "url": url,
                "type": if meta.pr_url.as_deref() == Some(url) { "pr" } else { "issue" }
            })
        );
    } else {
        let link_type = if url.contains("/pull/")
            || url.contains("/pulls/")
            || url.contains("/merge_requests/")
        {
            "PR"
        } else {
            "issue"
        };
        println!("Linked card {} to {link_type}: {url}", card_uuid);
    }
    Ok(())
}

// ─── PR ──────────────────────────────────────────────────────

pub fn pr(repo: &Path, card_id: &str, json_output: bool) -> Result<()> {
    let store = Store::new(repo);
    if !store.is_initialized() {
        return Err(PmError::KukNotInitialized);
    }

    if !git::is_git_repo(repo) {
        return Err(PmError::NotGitRepo);
    }

    let config = store.load_config()?;
    let mut board = store.load_board(&config.default_board)?;

    let card_uuid = board
        .resolve_card_id(card_id)
        .ok_or_else(|| PmError::CardNotFound(card_id.into()))?;

    let card = board
        .find_card(&card_uuid)
        .ok_or_else(|| PmError::CardNotFound(card_id.into()))?;

    let title = card.title.clone();
    let body = card
        .description
        .clone()
        .unwrap_or_else(|| format!("Card: {}", card.title));

    let pr_url = sync::create_pr(repo, &title, &body)?;

    // Update card metadata with PR URL
    let card = board
        .find_card_mut(&card_uuid)
        .ok_or_else(|| PmError::CardNotFound(card_id.into()))?;

    let mut meta = sync::get_pm_metadata(card);
    meta.pr_url = Some(pr_url.clone());
    sync::set_pm_metadata(card, &meta);
    card.updated_at = chrono::Utc::now();

    store.save_board(&board)?;

    if json_output {
        println!(
            "{}",
            serde_json::json!({
                "card_id": card_uuid,
                "pr_url": pr_url,
                "title": title
            })
        );
    } else {
        println!("Created PR: {pr_url}");
        println!("  Card: {title}");
    }
    Ok(())
}

// ─── Velocity ────────────────────────────────────────────────

pub fn velocity(repo: &Path, weeks: u32, _target: Option<&str>, json_output: bool) -> Result<()> {
    let store = Store::new(repo);
    if !store.is_initialized() {
        return Err(PmError::KukNotInitialized);
    }

    let boards = load_all_boards(&store)?;
    let report = reports::calculate_velocity(&boards, weeks);

    if json_output {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        print!("{}", reports::render_velocity_text(&report));
    }
    Ok(())
}

// ─── Burndown ────────────────────────────────────────────────

pub fn burndown(repo: &Path, sprint_name: Option<&str>, json_output: bool) -> Result<()> {
    let store = Store::new(repo);
    if !store.is_initialized() {
        return Err(PmError::KukNotInitialized);
    }

    let sprints = load_sprints(&store)?;

    let sprint = match sprint_name {
        Some(name) => sprints
            .iter()
            .find(|s| s.name == name)
            .ok_or_else(|| PmError::SprintNotFound(name.into()))?,
        None => sprints
            .iter()
            .find(|s| s.status == SprintStatus::Active)
            .ok_or(PmError::NoActiveSprint)?,
    };

    let boards = load_all_boards(&store)?;
    let report = reports::calculate_burndown(&boards, sprint);

    if json_output {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        print!("{}", reports::render_burndown_text(&report));
    }
    Ok(())
}

// ─── Roadmap ─────────────────────────────────────────────────

pub fn roadmap(repo: &Path, weeks: u32, json_output: bool) -> Result<()> {
    let store = Store::new(repo);
    if !store.is_initialized() {
        return Err(PmError::KukNotInitialized);
    }

    let boards = load_all_boards(&store)?;
    let sprints = load_sprints(&store)?;

    // Use recent velocity for projection
    let vel_report = reports::calculate_velocity(&boards, 4);
    let velocity = if vel_report.average > 0.0 {
        vel_report.average
    } else {
        1.0 // default assumption
    };

    let report = reports::calculate_roadmap(&boards, &sprints, weeks, velocity);

    if json_output {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        print!("{}", reports::render_roadmap_text(&report));
    }
    Ok(())
}

// ─── Release Notes ───────────────────────────────────────────

pub fn release_notes(repo: &Path, since: Option<&str>, json_output: bool) -> Result<()> {
    if !git::is_git_repo(repo) {
        return Err(PmError::NotGitRepo);
    }

    let since_ref = since.unwrap_or("last-tag");

    let commits = if since_ref == "last-tag" {
        // Find most recent tag, fall back to all recent commits
        match git::list_tags(repo) {
            Ok(tags) if !tags.is_empty() => {
                let tag = tags.last().unwrap();
                git::commits_since_ref(repo, tag)?
            }
            _ => git::recent_commits(repo, 50)?,
        }
    } else {
        git::commits_since_ref(repo, since_ref)?
    };

    let mut report = reports::categorize_commits(&commits);
    report.since = since_ref.to_string();

    if json_output {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        print!("{}", reports::render_release_notes_text(&report));
    }
    Ok(())
}

// ─── Sync ────────────────────────────────────────────────────

pub fn sync(repo: &Path, dry_run: bool, json_output: bool) -> Result<()> {
    sync::run_sync(repo, dry_run, json_output)?;
    Ok(())
}

// ─── Stats ───────────────────────────────────────────────────

pub fn stats(repo: &Path, json_output: bool) -> Result<()> {
    let store = Store::new(repo);
    if !store.is_initialized() {
        return Err(PmError::KukNotInitialized);
    }

    let config = store.load_config()?;
    let board = store.load_board(&config.default_board)?;
    let report = reports::calculate_stats(&board);

    if json_output {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        print!("{}", reports::render_stats_text(&report));
    }
    Ok(())
}

// ─── Sprint/board helpers ────────────────────────────────────

fn load_sprints(store: &Store) -> Result<Vec<Sprint>> {
    let path = store.kuk_dir().join("sprints.json");
    if !path.exists() {
        return Ok(Vec::new());
    }
    let data = std::fs::read_to_string(&path)?;
    Ok(serde_json::from_str(&data)?)
}

fn save_sprints(store: &Store, sprints: &[Sprint]) -> Result<()> {
    let json = serde_json::to_string_pretty(sprints)?;
    std::fs::write(store.kuk_dir().join("sprints.json"), json)?;
    Ok(())
}

fn load_all_boards(store: &Store) -> Result<Vec<kuk::model::Board>> {
    let board_names = store.list_boards()?;
    let mut boards = Vec::new();
    for name in &board_names {
        boards.push(store.load_board(name)?);
    }
    Ok(boards)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugify_simple_title() {
        assert_eq!(slugify_branch("Implement login"), "feature/implement-login");
    }

    #[test]
    fn slugify_special_chars() {
        assert_eq!(slugify_branch("Fix bug #42"), "feature/fix-bug-42");
    }

    #[test]
    fn slugify_collapses_dashes() {
        assert_eq!(
            slugify_branch("Add   spaces   here"),
            "feature/add-spaces-here"
        );
    }

    #[test]
    fn slugify_uppercase() {
        assert_eq!(slugify_branch("UPPER CASE"), "feature/upper-case");
    }

    #[test]
    fn slugify_already_clean() {
        assert_eq!(slugify_branch("clean-title"), "feature/clean-title");
    }
}
