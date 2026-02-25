use chrono::Utc;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

use crate::error::{KukError, Result};
use crate::model::{Card, Column};
use crate::storage::Store;

#[derive(Parser, Debug)]
#[command(name = "kuk", version, about = "Kanban that ships with your code.")]
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
    /// Initialize a new kuk board in the current repo
    Init {
        /// Name of the initial board
        #[arg(long, default_value = "default")]
        board_name: String,
    },

    /// List all cards on the board
    List {
        /// Board name (defaults to active board)
        #[arg(long)]
        board: Option<String>,
    },

    /// Add a new card
    Add {
        /// Card title
        title: String,
        /// Target column
        #[arg(long, default_value = "todo")]
        to: String,
        /// Labels to add
        #[arg(long)]
        label: Vec<String>,
        /// Assignee
        #[arg(long)]
        assignee: Option<String>,
    },

    /// Move a card to a different column
    Move {
        /// Card ID or number
        id: String,
        /// Target column
        #[arg(long)]
        to: String,
    },

    /// Move a card to the top of its column
    Hoist {
        /// Card ID or number
        id: String,
    },

    /// Move a card to the bottom of its column
    Demote {
        /// Card ID or number
        id: String,
    },

    /// Archive a card
    Archive {
        /// Card ID or number
        id: String,
    },

    /// Delete a card permanently
    Delete {
        /// Card ID or number
        id: String,
    },

    /// Add or remove labels from a card
    Label {
        /// Card ID or number
        id: String,
        /// Action: add or remove
        action: String,
        /// Tag name
        tag: String,
    },

    /// Assign a user to a card
    Assign {
        /// Card ID or number
        id: String,
        /// Username
        user: String,
    },

    /// Board management
    Board {
        #[command(subcommand)]
        command: BoardCmd,
    },

    /// List all kuk projects on this machine
    Projects,

    /// Launch the TUI
    Tui,

    /// Start the REST + MCP server
    Serve {
        /// Port to listen on
        #[arg(long, default_value = "8080")]
        port: u16,

        /// Enable MCP endpoint
        #[arg(long)]
        mcp: bool,
    },

    /// Run as MCP server (stdio transport for Claude Code / AI agents)
    Mcp,

    /// Health check
    Doctor,

    /// Show version
    Version,
}

#[derive(Subcommand, Debug)]
pub enum BoardCmd {
    /// Create a new board
    Create {
        /// Board name
        name: String,
    },
    /// Switch default board
    Switch {
        /// Board name
        name: String,
    },
    /// List all boards
    List,
}

// --- Command implementations ---

pub fn init(store: &Store, _board_name: &str) -> Result<()> {
    store.init()?;
    println!("Initialized kuk board in {}", store.kuk_dir().display());
    Ok(())
}

pub fn list(store: &Store, board_name: Option<&str>, json_output: bool) -> Result<()> {
    let config = store.load_config()?;
    let board_name = board_name.unwrap_or(&config.default_board);
    let board = store.load_board(board_name)?;

    if json_output {
        println!("{}", serde_json::to_string_pretty(&board)?);
        return Ok(());
    }

    for col in &board.columns {
        let cards: Vec<&Card> = board
            .cards
            .iter()
            .filter(|c| c.column == col.name && !c.archived)
            .collect();

        let wip = col
            .wip_limit
            .map(|l| format!(" [{}/{}]", cards.len(), l))
            .unwrap_or_default();

        println!("── {} ({}){}──", col.name.to_uppercase(), cards.len(), wip);

        let mut sorted = cards;
        sorted.sort_by_key(|c| c.order);

        for (i, card) in sorted.iter().enumerate() {
            let labels = if card.labels.is_empty() {
                String::new()
            } else {
                format!(" [{}]", card.labels.join(", "))
            };
            let assignee = card
                .assignee
                .as_ref()
                .map(|a| format!(" @{a}"))
                .unwrap_or_default();
            println!("  {}. {}{}{}", i + 1, card.title, labels, assignee);
        }
        println!();
    }
    Ok(())
}

pub fn add(
    store: &Store,
    title: &str,
    column: &str,
    labels: Vec<String>,
    assignee: Option<String>,
    json_output: bool,
) -> Result<()> {
    let config = store.load_config()?;
    let mut board = store.load_board(&config.default_board)?;

    if !board.has_column(column) {
        return Err(KukError::ColumnNotFound(column.into()));
    }

    let mut card = Card::new(title, column);
    card.order = board.next_order(column);
    card.labels = labels;
    card.assignee = assignee;

    if json_output {
        println!("{}", serde_json::to_string_pretty(&card)?);
    } else {
        println!("Added: {} → {}", card.title, card.column);
    }

    board.cards.push(card);
    store.save_board(&board)?;
    Ok(())
}

pub fn move_card(store: &Store, id_or_num: &str, to: &str, json_output: bool) -> Result<()> {
    let config = store.load_config()?;
    let mut board = store.load_board(&config.default_board)?;

    if !board.has_column(to) {
        return Err(KukError::ColumnNotFound(to.into()));
    }

    let card_id = board
        .resolve_card_id(id_or_num)
        .ok_or_else(|| KukError::CardNotFound(id_or_num.into()))?;

    let next_order = board.next_order(to);
    let card = board
        .find_card_mut(&card_id)
        .ok_or_else(|| KukError::CardNotFound(id_or_num.into()))?;

    card.column = to.into();
    card.order = next_order;
    card.updated_at = Utc::now();

    if json_output {
        println!("{}", serde_json::to_string_pretty(card)?);
    } else {
        println!("Moved: {} → {}", card.title, to);
    }

    store.save_board(&board)?;
    Ok(())
}

pub fn hoist(store: &Store, id_or_num: &str, json_output: bool) -> Result<()> {
    let config = store.load_config()?;
    let mut board = store.load_board(&config.default_board)?;

    let card_id = board
        .resolve_card_id(id_or_num)
        .ok_or_else(|| KukError::CardNotFound(id_or_num.into()))?;

    let column = board.find_card(&card_id).unwrap().column.clone();

    // Shift all other cards in the column down by 1
    for c in board.cards.iter_mut() {
        if c.column == column && !c.archived && c.id != card_id {
            c.order += 1;
        }
    }

    let card = board.find_card_mut(&card_id).unwrap();
    card.order = 0;
    card.updated_at = Utc::now();

    if json_output {
        println!("{}", serde_json::to_string_pretty(card)?);
    } else {
        println!("Hoisted: {} to top of {}", card.title, column);
    }

    store.save_board(&board)?;
    Ok(())
}

pub fn demote(store: &Store, id_or_num: &str, json_output: bool) -> Result<()> {
    let config = store.load_config()?;
    let mut board = store.load_board(&config.default_board)?;

    let card_id = board
        .resolve_card_id(id_or_num)
        .ok_or_else(|| KukError::CardNotFound(id_or_num.into()))?;

    let column = board.find_card(&card_id).unwrap().column.clone();
    let max_order = board.next_order(&column);

    let card = board.find_card_mut(&card_id).unwrap();
    card.order = max_order;
    card.updated_at = Utc::now();

    if json_output {
        println!("{}", serde_json::to_string_pretty(card)?);
    } else {
        println!("Demoted: {} to bottom of {}", card.title, column);
    }

    store.save_board(&board)?;
    Ok(())
}

pub fn archive(store: &Store, id_or_num: &str, json_output: bool) -> Result<()> {
    let config = store.load_config()?;
    let mut board = store.load_board(&config.default_board)?;

    let card_id = board
        .resolve_card_id(id_or_num)
        .ok_or_else(|| KukError::CardNotFound(id_or_num.into()))?;

    let card = board
        .find_card_mut(&card_id)
        .ok_or_else(|| KukError::CardNotFound(id_or_num.into()))?;

    card.archived = true;
    card.updated_at = Utc::now();

    if json_output {
        println!("{}", serde_json::to_string_pretty(card)?);
    } else {
        println!("Archived: {}", card.title);
    }

    store.save_board(&board)?;
    Ok(())
}

pub fn delete(store: &Store, id_or_num: &str, json_output: bool) -> Result<()> {
    let config = store.load_config()?;
    let mut board = store.load_board(&config.default_board)?;

    let card_id = board
        .resolve_card_id(id_or_num)
        .ok_or_else(|| KukError::CardNotFound(id_or_num.into()))?;

    let card = board
        .find_card(&card_id)
        .ok_or_else(|| KukError::CardNotFound(id_or_num.into()))?;

    let title = card.title.clone();
    board.cards.retain(|c| c.id != card_id);

    if json_output {
        println!(
            "{}",
            serde_json::json!({"deleted": card_id, "title": title})
        );
    } else {
        println!("Deleted: {}", title);
    }

    store.save_board(&board)?;
    Ok(())
}

pub fn label(
    store: &Store,
    id_or_num: &str,
    action: &str,
    tag: &str,
    json_output: bool,
) -> Result<()> {
    let config = store.load_config()?;
    let mut board = store.load_board(&config.default_board)?;

    let card_id = board
        .resolve_card_id(id_or_num)
        .ok_or_else(|| KukError::CardNotFound(id_or_num.into()))?;

    let card = board
        .find_card_mut(&card_id)
        .ok_or_else(|| KukError::CardNotFound(id_or_num.into()))?;

    match action {
        "add" => {
            if !card.labels.contains(&tag.to_string()) {
                card.labels.push(tag.to_string());
            }
        }
        "remove" => {
            if !card.labels.contains(&tag.to_string()) {
                return Err(KukError::LabelNotFound(tag.into()));
            }
            card.labels.retain(|l| l != tag);
        }
        _ => {
            return Err(KukError::Other(format!(
                "Invalid label action: {action}. Use 'add' or 'remove'."
            )));
        }
    }

    card.updated_at = Utc::now();

    if json_output {
        println!("{}", serde_json::to_string_pretty(card)?);
    } else {
        println!("Labels on {}: [{}]", card.title, card.labels.join(", "));
    }

    store.save_board(&board)?;
    Ok(())
}

pub fn assign(store: &Store, id_or_num: &str, user: &str, json_output: bool) -> Result<()> {
    let config = store.load_config()?;
    let mut board = store.load_board(&config.default_board)?;

    let card_id = board
        .resolve_card_id(id_or_num)
        .ok_or_else(|| KukError::CardNotFound(id_or_num.into()))?;

    let card = board
        .find_card_mut(&card_id)
        .ok_or_else(|| KukError::CardNotFound(id_or_num.into()))?;

    card.assignee = Some(user.into());
    card.updated_at = Utc::now();

    if json_output {
        println!("{}", serde_json::to_string_pretty(card)?);
    } else {
        println!("Assigned {} to @{}", card.title, user);
    }

    store.save_board(&board)?;
    Ok(())
}

pub fn board(store: &Store, cmd: BoardCmd, json_output: bool) -> Result<()> {
    match cmd {
        BoardCmd::Create { name } => {
            store.create_board(
                &name,
                vec![
                    Column {
                        name: "todo".into(),
                        wip_limit: None,
                    },
                    Column {
                        name: "doing".into(),
                        wip_limit: None,
                    },
                    Column {
                        name: "done".into(),
                        wip_limit: None,
                    },
                ],
            )?;
            if json_output {
                println!("{}", serde_json::json!({"created": name}));
            } else {
                println!("Created board: {}", name);
            }
        }
        BoardCmd::Switch { name } => {
            // Verify board exists
            store.load_board(&name)?;
            let mut config = store.load_config()?;
            config.default_board = name.clone();
            store.save_config(&config)?;
            if json_output {
                println!("{}", serde_json::json!({"active": name}));
            } else {
                println!("Switched to board: {}", name);
            }
        }
        BoardCmd::List => {
            let config = store.load_config()?;
            let boards = store.list_boards()?;
            if json_output {
                println!("{}", serde_json::to_string_pretty(&boards)?);
            } else {
                for b in &boards {
                    if *b == config.default_board {
                        println!("* {}", b);
                    } else {
                        println!("  {}", b);
                    }
                }
            }
        }
    }
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
        println!("  {} → {}", p.name, p.path);
    }
    Ok(())
}

pub fn doctor(store: &Store) -> Result<()> {
    println!("kuk doctor");
    println!("──────────");

    // Check .kuk exists
    if store.is_initialized() {
        println!("  [OK] .kuk/ directory found");
    } else {
        println!("  [!!] .kuk/ not found. Run `kuk init`.");
        return Ok(());
    }

    // Check config
    match store.load_config() {
        Ok(config) => println!("  [OK] config.json (v{})", config.version),
        Err(e) => println!("  [!!] config.json: {}", e),
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
                        println!("       └─ {}: {} active, {} archived", b, active, archived);
                    }
                    Err(e) => println!("       └─ {}: ERROR: {}", b, e),
                }
            }
        }
        Err(e) => println!("  [!!] boards: {}", e),
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
    println!("kuk {}", env!("CARGO_PKG_VERSION"));
    Ok(())
}

pub fn default_action() -> Result<()> {
    println!("kuk — Kanban that ships with your code.");
    println!();
    println!("Run `kuk --help` for usage or `kuk init` to get started.");
    Ok(())
}
