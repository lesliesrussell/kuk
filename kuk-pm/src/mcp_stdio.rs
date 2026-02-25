//! Stdio-based MCP (Model Context Protocol) server for kuk-pm.
//!
//! Reads JSON-RPC 2.0 messages from stdin (one per line),
//! processes them, and writes responses to stdout.
//! Exposes project management tools: stats, velocity, burndown,
//! roadmap, sprints, release notes, sync, and linking.

use std::io::{self, BufRead, Write};
use std::path::Path;

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::PmError;
use crate::git;
use crate::model::{Sprint, SprintStatus};
use crate::reports;
use crate::sync;
use kuk::model::Board;
use kuk::storage::Store;

#[derive(Debug, Deserialize)]
struct JsonRpcRequest {
    #[allow(dead_code)]
    jsonrpc: Option<String>,
    id: Option<Value>,
    method: String,
    #[serde(default)]
    params: Value,
}

#[derive(Debug, Serialize)]
struct JsonRpcResponse {
    jsonrpc: String,
    id: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<JsonRpcError>,
}

#[derive(Debug, Serialize)]
struct JsonRpcError {
    code: i32,
    message: String,
}

impl JsonRpcResponse {
    fn success(id: Value, result: Value) -> Self {
        Self {
            jsonrpc: "2.0".into(),
            id,
            result: Some(result),
            error: None,
        }
    }

    fn error(id: Value, code: i32, message: impl Into<String>) -> Self {
        Self {
            jsonrpc: "2.0".into(),
            id,
            result: None,
            error: Some(JsonRpcError {
                code,
                message: message.into(),
            }),
        }
    }
}

/// Run the stdio MCP server loop. Blocks until stdin is closed.
pub fn run(store: &Store, repo: &Path) -> crate::error::Result<()> {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let reader = stdin.lock();
    let mut writer = stdout.lock();

    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };

        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let req: JsonRpcRequest = match serde_json::from_str(trimmed) {
            Ok(r) => r,
            Err(e) => {
                let resp =
                    JsonRpcResponse::error(Value::Null, -32700, format!("Parse error: {e}"));
                let _ = writeln!(writer, "{}", serde_json::to_string(&resp).unwrap());
                let _ = writer.flush();
                continue;
            }
        };

        let is_notification = req.id.is_none();
        let id = req.id.clone().unwrap_or(Value::Null);

        let response = match req.method.as_str() {
            "initialize" => Some(handle_initialize(id)),
            "notifications/initialized" | "initialized" => None,
            "tools/list" => Some(handle_tools_list(id)),
            "tools/call" => Some(handle_tools_call(id, &req.params, store, repo)),
            "ping" => Some(JsonRpcResponse::success(id, serde_json::json!({}))),
            _ => {
                if is_notification {
                    None
                } else {
                    Some(JsonRpcResponse::error(
                        id,
                        -32601,
                        format!("Method not found: {}", req.method),
                    ))
                }
            }
        };

        if let Some(resp) = response {
            let _ = writeln!(writer, "{}", serde_json::to_string(&resp).unwrap());
            let _ = writer.flush();
        }
    }

    Ok(())
}

fn handle_initialize(id: Value) -> JsonRpcResponse {
    JsonRpcResponse::success(
        id,
        serde_json::json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {
                "tools": {}
            },
            "serverInfo": {
                "name": "kuk-pm",
                "version": env!("CARGO_PKG_VERSION")
            }
        }),
    )
}

fn handle_tools_list(id: Value) -> JsonRpcResponse {
    let tools = serde_json::json!({
        "tools": [
            {
                "name": "pm_stats",
                "description": "Show project statistics (cards per column, label distribution, aging)",
                "inputSchema": {
                    "type": "object",
                    "properties": {}
                }
            },
            {
                "name": "pm_velocity",
                "description": "Show velocity metrics (cards completed per week)",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "weeks": {"type": "number", "description": "Number of weeks to analyze (default: 4)"}
                    }
                }
            },
            {
                "name": "pm_burndown",
                "description": "Show burndown chart for a sprint",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "sprint": {"type": "string", "description": "Sprint name (default: active sprint)"}
                    }
                }
            },
            {
                "name": "pm_roadmap",
                "description": "Show roadmap projection based on velocity",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "weeks": {"type": "number", "description": "Number of weeks to project (default: 12)"}
                    }
                }
            },
            {
                "name": "pm_sprint_list",
                "description": "List all sprints with their status",
                "inputSchema": {
                    "type": "object",
                    "properties": {}
                }
            },
            {
                "name": "pm_sprint_create",
                "description": "Create a new sprint",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "name": {"type": "string", "description": "Sprint name"},
                        "start": {"type": "string", "description": "Start date (YYYY-MM-DD)"},
                        "end": {"type": "string", "description": "End date (YYYY-MM-DD)"}
                    },
                    "required": ["name", "start", "end"]
                }
            },
            {
                "name": "pm_sprint_start",
                "description": "Start a planned sprint (set status to active)",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "name": {"type": "string", "description": "Sprint name"}
                    },
                    "required": ["name"]
                }
            },
            {
                "name": "pm_sprint_end",
                "description": "End/close an active sprint",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "name": {"type": "string", "description": "Sprint name"}
                    },
                    "required": ["name"]
                }
            },
            {
                "name": "pm_link",
                "description": "Link a kanban card to a GitHub issue or PR URL",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "card_id": {"type": "string", "description": "Card ID or short number (e.g. #1)"},
                        "url": {"type": "string", "description": "GitHub issue or PR URL"}
                    },
                    "required": ["card_id", "url"]
                }
            },
            {
                "name": "pm_release_notes",
                "description": "Generate release notes from git commit history",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "since": {"type": "string", "description": "Starting point - tag or ref (default: last tag)"}
                    }
                }
            },
            {
                "name": "pm_sync",
                "description": "Sync kanban board with GitHub issues/PRs",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "dry_run": {"type": "boolean", "description": "Preview changes without applying (default: false)"}
                    }
                }
            }
        ]
    });
    JsonRpcResponse::success(id, tools)
}

fn handle_tools_call(
    id: Value,
    params: &Value,
    store: &Store,
    repo: &Path,
) -> JsonRpcResponse {
    let tool_name = params["name"].as_str().unwrap_or("");
    let args = &params["arguments"];

    match tool_name {
        "pm_stats" => tool_stats(id, store),
        "pm_velocity" => tool_velocity(id, args, store),
        "pm_burndown" => tool_burndown(id, args, store),
        "pm_roadmap" => tool_roadmap(id, args, store),
        "pm_sprint_list" => tool_sprint_list(id, store),
        "pm_sprint_create" => tool_sprint_create(id, args, store),
        "pm_sprint_start" => tool_sprint_start(id, args, store),
        "pm_sprint_end" => tool_sprint_end(id, args, store),
        "pm_link" => tool_link(id, args, store),
        "pm_release_notes" => tool_release_notes(id, args, repo),
        "pm_sync" => tool_sync(id, args, repo),
        _ => JsonRpcResponse::error(id, -32602, format!("Unknown tool: {tool_name}")),
    }
}

fn text_content(text: &str) -> Value {
    serde_json::json!({
        "content": [{"type": "text", "text": text}]
    })
}

// ─── Helper functions ────────────────────────────────────────

fn load_sprints(store: &Store) -> Result<Vec<Sprint>, PmError> {
    let path = store.kuk_dir().join("sprints.json");
    if !path.exists() {
        return Ok(Vec::new());
    }
    let data = std::fs::read_to_string(&path)?;
    Ok(serde_json::from_str(&data)?)
}

fn save_sprints(store: &Store, sprints: &[Sprint]) -> Result<(), PmError> {
    let json = serde_json::to_string_pretty(sprints)?;
    std::fs::write(store.kuk_dir().join("sprints.json"), json)?;
    Ok(())
}

fn load_all_boards(store: &Store) -> Result<Vec<Board>, PmError> {
    let board_names = store.list_boards()?;
    let mut boards = Vec::new();
    for name in &board_names {
        boards.push(store.load_board(name)?);
    }
    Ok(boards)
}

// ─── Tool implementations ────────────────────────────────────

fn tool_stats(id: Value, store: &Store) -> JsonRpcResponse {
    if !store.is_initialized() {
        return JsonRpcResponse::error(id, -32603, "kuk not initialized");
    }

    let config = match store.load_config() {
        Ok(c) => c,
        Err(e) => return JsonRpcResponse::error(id, -32603, e.to_string()),
    };
    let board = match store.load_board(&config.default_board) {
        Ok(b) => b,
        Err(e) => return JsonRpcResponse::error(id, -32603, e.to_string()),
    };

    let report = reports::calculate_stats(&board);
    let json = serde_json::to_string_pretty(&report).unwrap_or_default();
    JsonRpcResponse::success(id, text_content(&json))
}

fn tool_velocity(id: Value, args: &Value, store: &Store) -> JsonRpcResponse {
    if !store.is_initialized() {
        return JsonRpcResponse::error(id, -32603, "kuk not initialized");
    }

    let weeks = args["weeks"].as_u64().unwrap_or(4) as u32;

    let boards = match load_all_boards(store) {
        Ok(b) => b,
        Err(e) => return JsonRpcResponse::error(id, -32603, e.to_string()),
    };

    let report = reports::calculate_velocity(&boards, weeks);
    let json = serde_json::to_string_pretty(&report).unwrap_or_default();
    JsonRpcResponse::success(id, text_content(&json))
}

fn tool_burndown(id: Value, args: &Value, store: &Store) -> JsonRpcResponse {
    if !store.is_initialized() {
        return JsonRpcResponse::error(id, -32603, "kuk not initialized");
    }

    let sprints = match load_sprints(store) {
        Ok(s) => s,
        Err(e) => return JsonRpcResponse::error(id, -32603, e.to_string()),
    };

    let sprint_name = args["sprint"].as_str();
    let sprint = match sprint_name {
        Some(name) => match sprints.iter().find(|s| s.name == name) {
            Some(s) => s,
            None => return JsonRpcResponse::error(id, -32602, format!("Sprint not found: {name}")),
        },
        None => match sprints.iter().find(|s| s.status == SprintStatus::Active) {
            Some(s) => s,
            None => return JsonRpcResponse::error(id, -32602, "No active sprint found"),
        },
    };

    let boards = match load_all_boards(store) {
        Ok(b) => b,
        Err(e) => return JsonRpcResponse::error(id, -32603, e.to_string()),
    };

    let report = reports::calculate_burndown(&boards, sprint);
    let json = serde_json::to_string_pretty(&report).unwrap_or_default();
    JsonRpcResponse::success(id, text_content(&json))
}

fn tool_roadmap(id: Value, args: &Value, store: &Store) -> JsonRpcResponse {
    if !store.is_initialized() {
        return JsonRpcResponse::error(id, -32603, "kuk not initialized");
    }

    let weeks = args["weeks"].as_u64().unwrap_or(12) as u32;

    let boards = match load_all_boards(store) {
        Ok(b) => b,
        Err(e) => return JsonRpcResponse::error(id, -32603, e.to_string()),
    };
    let sprints = match load_sprints(store) {
        Ok(s) => s,
        Err(e) => return JsonRpcResponse::error(id, -32603, e.to_string()),
    };

    let vel_report = reports::calculate_velocity(&boards, 4);
    let velocity = if vel_report.average > 0.0 {
        vel_report.average
    } else {
        1.0
    };

    let report = reports::calculate_roadmap(&boards, &sprints, weeks, velocity);
    let json = serde_json::to_string_pretty(&report).unwrap_or_default();
    JsonRpcResponse::success(id, text_content(&json))
}

fn tool_sprint_list(id: Value, store: &Store) -> JsonRpcResponse {
    let sprints = match load_sprints(store) {
        Ok(s) => s,
        Err(e) => return JsonRpcResponse::error(id, -32603, e.to_string()),
    };

    if sprints.is_empty() {
        return JsonRpcResponse::success(
            id,
            text_content("No sprints defined. Use pm_sprint_create to create one."),
        );
    }

    let json = serde_json::to_string_pretty(&sprints).unwrap_or_default();
    JsonRpcResponse::success(id, text_content(&json))
}

fn tool_sprint_create(id: Value, args: &Value, store: &Store) -> JsonRpcResponse {
    let name = match args["name"].as_str() {
        Some(n) => n,
        None => return JsonRpcResponse::error(id, -32602, "name is required"),
    };
    let start_str = match args["start"].as_str() {
        Some(s) => s,
        None => return JsonRpcResponse::error(id, -32602, "start date is required"),
    };
    let end_str = match args["end"].as_str() {
        Some(e) => e,
        None => return JsonRpcResponse::error(id, -32602, "end date is required"),
    };

    let start = match NaiveDate::parse_from_str(start_str, "%Y-%m-%d") {
        Ok(d) => d,
        Err(_) => {
            return JsonRpcResponse::error(
                id,
                -32602,
                format!("Invalid start date: {start_str} (expected YYYY-MM-DD)"),
            )
        }
    };
    let end = match NaiveDate::parse_from_str(end_str, "%Y-%m-%d") {
        Ok(d) => d,
        Err(_) => {
            return JsonRpcResponse::error(
                id,
                -32602,
                format!("Invalid end date: {end_str} (expected YYYY-MM-DD)"),
            )
        }
    };

    if end <= start {
        return JsonRpcResponse::error(id, -32602, "end date must be after start date");
    }

    let mut sprints = match load_sprints(store) {
        Ok(s) => s,
        Err(e) => return JsonRpcResponse::error(id, -32603, e.to_string()),
    };

    if sprints.iter().any(|s| s.name == name) {
        return JsonRpcResponse::error(id, -32602, format!("Sprint already exists: {name}"));
    }

    let sprint = Sprint {
        name: name.into(),
        start,
        end,
        goal: None,
        boards: Vec::new(),
        status: SprintStatus::Planned,
    };

    sprints.push(sprint);
    if let Err(e) = save_sprints(store, &sprints) {
        return JsonRpcResponse::error(id, -32603, e.to_string());
    }

    JsonRpcResponse::success(
        id,
        text_content(&format!("Created sprint: {name} ({start} -> {end})")),
    )
}

fn tool_sprint_start(id: Value, args: &Value, store: &Store) -> JsonRpcResponse {
    let name = match args["name"].as_str() {
        Some(n) => n,
        None => return JsonRpcResponse::error(id, -32602, "name is required"),
    };

    let mut sprints = match load_sprints(store) {
        Ok(s) => s,
        Err(e) => return JsonRpcResponse::error(id, -32603, e.to_string()),
    };

    let sprint = match sprints.iter_mut().find(|s| s.name == name) {
        Some(s) => s,
        None => return JsonRpcResponse::error(id, -32602, format!("Sprint not found: {name}")),
    };

    match sprint.status {
        SprintStatus::Active => {
            return JsonRpcResponse::error(id, -32602, format!("Sprint already active: {name}"))
        }
        SprintStatus::Closed => {
            return JsonRpcResponse::error(id, -32602, format!("Sprint already closed: {name}"))
        }
        SprintStatus::Planned => {}
    }

    sprint.status = SprintStatus::Active;

    if let Err(e) = save_sprints(store, &sprints) {
        return JsonRpcResponse::error(id, -32603, e.to_string());
    }

    JsonRpcResponse::success(id, text_content(&format!("Started sprint: {name}")))
}

fn tool_sprint_end(id: Value, args: &Value, store: &Store) -> JsonRpcResponse {
    let name = match args["name"].as_str() {
        Some(n) => n,
        None => return JsonRpcResponse::error(id, -32602, "name is required"),
    };

    let mut sprints = match load_sprints(store) {
        Ok(s) => s,
        Err(e) => return JsonRpcResponse::error(id, -32603, e.to_string()),
    };

    let sprint = match sprints.iter_mut().find(|s| s.name == name) {
        Some(s) => s,
        None => return JsonRpcResponse::error(id, -32602, format!("Sprint not found: {name}")),
    };

    if sprint.status == SprintStatus::Closed {
        return JsonRpcResponse::error(id, -32602, format!("Sprint already closed: {name}"));
    }

    sprint.status = SprintStatus::Closed;

    if let Err(e) = save_sprints(store, &sprints) {
        return JsonRpcResponse::error(id, -32603, e.to_string());
    }

    JsonRpcResponse::success(id, text_content(&format!("Closed sprint: {name}")))
}

fn tool_link(id: Value, args: &Value, store: &Store) -> JsonRpcResponse {
    if !store.is_initialized() {
        return JsonRpcResponse::error(id, -32603, "kuk not initialized");
    }

    let card_id = match args["card_id"].as_str() {
        Some(c) => c,
        None => return JsonRpcResponse::error(id, -32602, "card_id is required"),
    };
    let url = match args["url"].as_str() {
        Some(u) => u,
        None => return JsonRpcResponse::error(id, -32602, "url is required"),
    };

    let config = match store.load_config() {
        Ok(c) => c,
        Err(e) => return JsonRpcResponse::error(id, -32603, e.to_string()),
    };
    let mut board = match store.load_board(&config.default_board) {
        Ok(b) => b,
        Err(e) => return JsonRpcResponse::error(id, -32603, e.to_string()),
    };

    let card_uuid = match board.resolve_card_id(card_id) {
        Some(id) => id,
        None => return JsonRpcResponse::error(id, -32602, format!("Card not found: {card_id}")),
    };

    let card = match board.find_card_mut(&card_uuid) {
        Some(c) => c,
        None => return JsonRpcResponse::error(id, -32602, format!("Card not found: {card_id}")),
    };

    let mut meta = sync::get_pm_metadata(card);

    let link_type =
        if url.contains("/pull/") || url.contains("/pulls/") || url.contains("/merge_requests/") {
            meta.pr_url = Some(url.into());
            "PR"
        } else {
            meta.issue_url = Some(url.into());
            "issue"
        };

    sync::set_pm_metadata(card, &meta);
    card.updated_at = chrono::Utc::now();

    if let Err(e) = store.save_board(&board) {
        return JsonRpcResponse::error(id, -32603, e.to_string());
    }

    JsonRpcResponse::success(
        id,
        text_content(&format!("Linked card {card_id} to {link_type}: {url}")),
    )
}

fn tool_release_notes(id: Value, args: &Value, repo: &Path) -> JsonRpcResponse {
    if !git::is_git_repo(repo) {
        return JsonRpcResponse::error(id, -32603, "Not a git repository");
    }

    let since_ref = args["since"].as_str().unwrap_or("last-tag");

    let commits = if since_ref == "last-tag" {
        match git::list_tags(repo) {
            Ok(tags) if !tags.is_empty() => {
                let tag = tags.last().unwrap();
                match git::commits_since_ref(repo, tag) {
                    Ok(c) => c,
                    Err(e) => return JsonRpcResponse::error(id, -32603, e.to_string()),
                }
            }
            _ => match git::recent_commits(repo, 50) {
                Ok(c) => c,
                Err(e) => return JsonRpcResponse::error(id, -32603, e.to_string()),
            },
        }
    } else {
        match git::commits_since_ref(repo, since_ref) {
            Ok(c) => c,
            Err(e) => return JsonRpcResponse::error(id, -32603, e.to_string()),
        }
    };

    let mut report = reports::categorize_commits(&commits);
    report.since = since_ref.to_string();

    let json = serde_json::to_string_pretty(&report).unwrap_or_default();
    JsonRpcResponse::success(id, text_content(&json))
}

fn tool_sync(id: Value, args: &Value, repo: &Path) -> JsonRpcResponse {
    let dry_run = args["dry_run"].as_bool().unwrap_or(false);

    match sync::run_sync(repo, dry_run, true) {
        Ok(actions) => {
            let json = serde_json::to_string_pretty(&actions).unwrap_or_default();
            JsonRpcResponse::success(id, text_content(&json))
        }
        Err(e) => JsonRpcResponse::error(id, -32603, e.to_string()),
    }
}
