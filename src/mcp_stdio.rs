//! Stdio-based MCP (Model Context Protocol) server.
//!
//! Reads JSON-RPC 2.0 messages from stdin (one per line),
//! processes them, and writes responses to stdout.
//! This is the transport Claude Code uses for local MCP servers.

use std::io::{self, BufRead, Write};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::model::Card;
use crate::storage::Store;

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
pub fn run(store: &Store) -> crate::error::Result<()> {
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
                let resp = JsonRpcResponse::error(Value::Null, -32700, format!("Parse error: {e}"));
                let _ = writeln!(writer, "{}", serde_json::to_string(&resp).unwrap());
                let _ = writer.flush();
                continue;
            }
        };

        // Notifications have no id — don't send a response
        let is_notification = req.id.is_none();
        let id = req.id.clone().unwrap_or(Value::Null);

        let response = match req.method.as_str() {
            "initialize" => Some(handle_initialize(id)),
            "notifications/initialized" | "initialized" => None,
            "tools/list" => Some(handle_tools_list(id)),
            "tools/call" => Some(handle_tools_call(id, &req.params, store)),
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
                "name": "kuk",
                "version": env!("CARGO_PKG_VERSION")
            }
        }),
    )
}

fn handle_tools_list(id: Value) -> JsonRpcResponse {
    let tools = serde_json::json!({
        "tools": [
            {
                "name": "kuk_add_card",
                "description": "Add a new card to the kanban board",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "title": {"type": "string", "description": "Card title"},
                        "column": {"type": "string", "description": "Target column (default: todo)"},
                        "labels": {"type": "array", "items": {"type": "string"}, "description": "Labels to attach"},
                        "assignee": {"type": "string", "description": "Assignee username"},
                        "board": {"type": "string", "description": "Board name (default: default)"}
                    },
                    "required": ["title"]
                }
            },
            {
                "name": "kuk_list_cards",
                "description": "List all cards on the board, grouped by column",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "board": {"type": "string", "description": "Board name (default: default)"}
                    }
                }
            },
            {
                "name": "kuk_move_card",
                "description": "Move a card to a different column",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "id": {"type": "string", "description": "Card ID or short number (e.g. #1)"},
                        "to": {"type": "string", "description": "Target column name"},
                        "board": {"type": "string", "description": "Board name (default: default)"}
                    },
                    "required": ["id", "to"]
                }
            },
            {
                "name": "kuk_archive_card",
                "description": "Archive a card (hide from board but keep data)",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "id": {"type": "string", "description": "Card ID or short number"},
                        "board": {"type": "string", "description": "Board name (default: default)"}
                    },
                    "required": ["id"]
                }
            },
            {
                "name": "kuk_delete_card",
                "description": "Permanently delete a card",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "id": {"type": "string", "description": "Card ID or short number"},
                        "board": {"type": "string", "description": "Board name (default: default)"}
                    },
                    "required": ["id"]
                }
            },
            {
                "name": "kuk_list_boards",
                "description": "List all kanban boards in this repository",
                "inputSchema": {
                    "type": "object",
                    "properties": {}
                }
            },
            {
                "name": "kuk_board_info",
                "description": "Get detailed info about a board including columns and card counts",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "board": {"type": "string", "description": "Board name (default: default)"}
                    }
                }
            }
        ]
    });
    JsonRpcResponse::success(id, tools)
}

fn handle_tools_call(id: Value, params: &Value, store: &Store) -> JsonRpcResponse {
    let tool_name = params["name"].as_str().unwrap_or("");
    let args = &params["arguments"];

    match tool_name {
        "kuk_add_card" => tool_add_card(id, args, store),
        "kuk_list_cards" => tool_list_cards(id, args, store),
        "kuk_move_card" => tool_move_card(id, args, store),
        "kuk_archive_card" => tool_archive_card(id, args, store),
        "kuk_delete_card" => tool_delete_card(id, args, store),
        "kuk_list_boards" => tool_list_boards(id, store),
        "kuk_board_info" => tool_board_info(id, args, store),
        _ => JsonRpcResponse::error(id, -32602, format!("Unknown tool: {tool_name}")),
    }
}

fn text_content(text: &str) -> Value {
    serde_json::json!({
        "content": [{"type": "text", "text": text}]
    })
}

fn tool_add_card(id: Value, args: &Value, store: &Store) -> JsonRpcResponse {
    let title = match args["title"].as_str() {
        Some(t) => t,
        None => return JsonRpcResponse::error(id, -32602, "title is required"),
    };
    let column = args["column"].as_str().unwrap_or("todo");
    let board_name = args["board"].as_str().unwrap_or("default");

    let mut board = match store.load_board(board_name) {
        Ok(b) => b,
        Err(e) => return JsonRpcResponse::error(id, -32603, e.to_string()),
    };

    if !board.has_column(column) {
        return JsonRpcResponse::error(id, -32602, format!("Column not found: {column}"));
    }

    let mut card = Card::new(title, column);
    card.order = board.next_order(column);

    if let Some(labels) = args["labels"].as_array() {
        card.labels = labels
            .iter()
            .filter_map(|v| v.as_str().map(String::from))
            .collect();
    }
    if let Some(assignee) = args["assignee"].as_str() {
        card.assignee = Some(assignee.into());
    }

    let result = serde_json::to_string_pretty(&card).unwrap();
    board.cards.push(card);

    if let Err(e) = store.save_board(&board) {
        return JsonRpcResponse::error(id, -32603, e.to_string());
    }

    JsonRpcResponse::success(id, text_content(&result))
}

fn tool_list_cards(id: Value, args: &Value, store: &Store) -> JsonRpcResponse {
    let board_name = args["board"].as_str().unwrap_or("default");

    match store.load_board(board_name) {
        Ok(board) => {
            // Format as a readable summary rather than raw JSON
            let mut lines = Vec::new();
            for col in &board.columns {
                let cards: Vec<&Card> = board
                    .cards
                    .iter()
                    .filter(|c| c.column == col.name && !c.archived)
                    .collect();
                lines.push(format!("## {} ({})", col.name, cards.len()));
                for (i, card) in cards.iter().enumerate() {
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
                    lines.push(format!(
                        "  {}. {} ({}){}{}", i + 1, card.title, &card.id[..8], labels, assignee
                    ));
                }
                if cards.is_empty() {
                    lines.push("  (empty)".into());
                }
                lines.push(String::new());
            }
            JsonRpcResponse::success(id, text_content(&lines.join("\n")))
        }
        Err(e) => JsonRpcResponse::error(id, -32603, e.to_string()),
    }
}

fn tool_move_card(id: Value, args: &Value, store: &Store) -> JsonRpcResponse {
    let card_id_str = match args["id"].as_str() {
        Some(s) => s,
        None => return JsonRpcResponse::error(id, -32602, "id is required"),
    };
    let to = match args["to"].as_str() {
        Some(s) => s,
        None => return JsonRpcResponse::error(id, -32602, "to is required"),
    };
    let board_name = args["board"].as_str().unwrap_or("default");

    let mut board = match store.load_board(board_name) {
        Ok(b) => b,
        Err(e) => return JsonRpcResponse::error(id, -32603, e.to_string()),
    };

    if !board.has_column(to) {
        return JsonRpcResponse::error(id, -32602, format!("Column not found: {to}"));
    }

    let resolved = match board.resolve_card_id(card_id_str) {
        Some(id) => id,
        None => {
            return JsonRpcResponse::error(id, -32602, format!("Card not found: {card_id_str}"))
        }
    };

    let next_order = board.next_order(to);
    let card = board.find_card_mut(&resolved).unwrap();
    card.column = to.into();
    card.order = next_order;
    card.updated_at = chrono::Utc::now();
    let title = card.title.clone();

    if let Err(e) = store.save_board(&board) {
        return JsonRpcResponse::error(id, -32603, e.to_string());
    }

    JsonRpcResponse::success(id, text_content(&format!("Moved \"{title}\" to {to}")))
}

fn tool_archive_card(id: Value, args: &Value, store: &Store) -> JsonRpcResponse {
    let card_id_str = match args["id"].as_str() {
        Some(s) => s,
        None => return JsonRpcResponse::error(id, -32602, "id is required"),
    };
    let board_name = args["board"].as_str().unwrap_or("default");

    let mut board = match store.load_board(board_name) {
        Ok(b) => b,
        Err(e) => return JsonRpcResponse::error(id, -32603, e.to_string()),
    };

    let resolved = match board.resolve_card_id(card_id_str) {
        Some(id) => id,
        None => {
            return JsonRpcResponse::error(id, -32602, format!("Card not found: {card_id_str}"))
        }
    };

    let card = board.find_card_mut(&resolved).unwrap();
    card.archived = true;
    card.updated_at = chrono::Utc::now();
    let title = card.title.clone();

    if let Err(e) = store.save_board(&board) {
        return JsonRpcResponse::error(id, -32603, e.to_string());
    }

    JsonRpcResponse::success(id, text_content(&format!("Archived \"{title}\"")))
}

fn tool_delete_card(id: Value, args: &Value, store: &Store) -> JsonRpcResponse {
    let card_id_str = match args["id"].as_str() {
        Some(s) => s,
        None => return JsonRpcResponse::error(id, -32602, "id is required"),
    };
    let board_name = args["board"].as_str().unwrap_or("default");

    let mut board = match store.load_board(board_name) {
        Ok(b) => b,
        Err(e) => return JsonRpcResponse::error(id, -32603, e.to_string()),
    };

    let resolved = match board.resolve_card_id(card_id_str) {
        Some(id) => id,
        None => {
            return JsonRpcResponse::error(id, -32602, format!("Card not found: {card_id_str}"))
        }
    };

    let title = board
        .find_card(&resolved)
        .map(|c| c.title.clone())
        .unwrap_or_default();
    board.cards.retain(|c| c.id != resolved);

    if let Err(e) = store.save_board(&board) {
        return JsonRpcResponse::error(id, -32603, e.to_string());
    }

    JsonRpcResponse::success(id, text_content(&format!("Deleted \"{title}\"")))
}

fn tool_list_boards(id: Value, store: &Store) -> JsonRpcResponse {
    match store.list_boards() {
        Ok(boards) => {
            let text = if boards.is_empty() {
                "No boards found.".into()
            } else {
                boards.join(", ")
            };
            JsonRpcResponse::success(id, text_content(&text))
        }
        Err(e) => JsonRpcResponse::error(id, -32603, e.to_string()),
    }
}

fn tool_board_info(id: Value, args: &Value, store: &Store) -> JsonRpcResponse {
    let board_name = args["board"].as_str().unwrap_or("default");

    match store.load_board(board_name) {
        Ok(board) => {
            let mut lines = vec![format!("Board: {}", board.name)];
            lines.push(format!(
                "Columns: {}",
                board
                    .columns
                    .iter()
                    .map(|c| c.name.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
            let active = board.cards.iter().filter(|c| !c.archived).count();
            let archived = board.cards.iter().filter(|c| c.archived).count();
            lines.push(format!("Cards: {active} active, {archived} archived"));
            lines.push(String::new());
            for col in &board.columns {
                let count = board
                    .cards
                    .iter()
                    .filter(|c| c.column == col.name && !c.archived)
                    .count();
                lines.push(format!("  {}: {count}", col.name));
            }
            JsonRpcResponse::success(id, text_content(&lines.join("\n")))
        }
        Err(e) => JsonRpcResponse::error(id, -32603, e.to_string()),
    }
}
