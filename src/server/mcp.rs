use std::sync::{Arc, Mutex};

use axum::extract::State;
use axum::response::Json;
use serde::{Deserialize, Serialize};

use crate::model::Card;
use crate::storage::Store;

type SharedStore = Arc<Mutex<Store>>;

/// Minimal MCP (Model Context Protocol) JSON-RPC handler.
/// Supports: tools/list, tools/call
#[derive(Debug, Deserialize)]
pub struct McpRequest {
    #[allow(dead_code)]
    pub jsonrpc: String,
    pub id: serde_json::Value,
    pub method: String,
    #[serde(default)]
    pub params: serde_json::Value,
}

#[derive(Debug, Serialize)]
pub struct McpResponse {
    pub jsonrpc: String,
    pub id: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<McpError>,
}

#[derive(Debug, Serialize)]
pub struct McpError {
    pub code: i32,
    pub message: String,
}

impl McpResponse {
    fn success(id: serde_json::Value, result: serde_json::Value) -> Self {
        Self {
            jsonrpc: "2.0".into(),
            id,
            result: Some(result),
            error: None,
        }
    }

    fn error(id: serde_json::Value, code: i32, message: impl Into<String>) -> Self {
        Self {
            jsonrpc: "2.0".into(),
            id,
            result: None,
            error: Some(McpError {
                code,
                message: message.into(),
            }),
        }
    }
}

pub async fn mcp_handler(
    State(store): State<SharedStore>,
    Json(req): Json<McpRequest>,
) -> Json<McpResponse> {
    let response = match req.method.as_str() {
        "tools/list" => handle_tools_list(req.id),
        "tools/call" => handle_tools_call(req.id, req.params, &store),
        _ => McpResponse::error(req.id, -32601, "Method not found"),
    };
    Json(response)
}

fn handle_tools_list(id: serde_json::Value) -> McpResponse {
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
                        "labels": {"type": "array", "items": {"type": "string"}, "description": "Labels"},
                        "assignee": {"type": "string", "description": "Assignee username"},
                        "board": {"type": "string", "description": "Board name (default: default)"}
                    },
                    "required": ["title"]
                }
            },
            {
                "name": "kuk_list_cards",
                "description": "List all cards on the board",
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
                        "id": {"type": "string", "description": "Card ID or number"},
                        "to": {"type": "string", "description": "Target column"},
                        "board": {"type": "string", "description": "Board name (default: default)"}
                    },
                    "required": ["id", "to"]
                }
            },
            {
                "name": "kuk_archive_card",
                "description": "Archive a card",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "id": {"type": "string", "description": "Card ID or number"}
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
                        "id": {"type": "string", "description": "Card ID or number"}
                    },
                    "required": ["id"]
                }
            }
        ]
    });
    McpResponse::success(id, tools)
}

fn handle_tools_call(
    id: serde_json::Value,
    params: serde_json::Value,
    store: &SharedStore,
) -> McpResponse {
    let tool_name = params["name"].as_str().unwrap_or("");
    let args = &params["arguments"];

    match tool_name {
        "kuk_add_card" => tool_add_card(id, args, store),
        "kuk_list_cards" => tool_list_cards(id, args, store),
        "kuk_move_card" => tool_move_card(id, args, store),
        "kuk_archive_card" => tool_archive_card(id, args, store),
        "kuk_delete_card" => tool_delete_card(id, args, store),
        _ => McpResponse::error(id, -32602, format!("Unknown tool: {tool_name}")),
    }
}

fn text_content(text: &str) -> serde_json::Value {
    serde_json::json!({
        "content": [{"type": "text", "text": text}]
    })
}

fn tool_add_card(
    id: serde_json::Value,
    args: &serde_json::Value,
    store: &SharedStore,
) -> McpResponse {
    let title = match args["title"].as_str() {
        Some(t) => t,
        None => return McpResponse::error(id, -32602, "title is required"),
    };
    let column = args["column"].as_str().unwrap_or("todo");
    let board_name = args["board"].as_str().unwrap_or("default");

    let store = store.lock().unwrap();
    let mut board = match store.load_board(board_name) {
        Ok(b) => b,
        Err(e) => return McpResponse::error(id, -32603, e.to_string()),
    };

    if !board.has_column(column) {
        return McpResponse::error(id, -32602, format!("Column not found: {column}"));
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
        return McpResponse::error(id, -32603, e.to_string());
    }

    McpResponse::success(id, text_content(&result))
}

fn tool_list_cards(
    id: serde_json::Value,
    args: &serde_json::Value,
    store: &SharedStore,
) -> McpResponse {
    let board_name = args["board"].as_str().unwrap_or("default");
    let store = store.lock().unwrap();

    match store.load_board(board_name) {
        Ok(board) => {
            let json = serde_json::to_string_pretty(&board).unwrap();
            McpResponse::success(id, text_content(&json))
        }
        Err(e) => McpResponse::error(id, -32603, e.to_string()),
    }
}

fn tool_move_card(
    id: serde_json::Value,
    args: &serde_json::Value,
    store: &SharedStore,
) -> McpResponse {
    let card_id_str = match args["id"].as_str() {
        Some(s) => s,
        None => return McpResponse::error(id, -32602, "id is required"),
    };
    let to = match args["to"].as_str() {
        Some(s) => s,
        None => return McpResponse::error(id, -32602, "to is required"),
    };
    let board_name = args["board"].as_str().unwrap_or("default");

    let store = store.lock().unwrap();
    let mut board = match store.load_board(board_name) {
        Ok(b) => b,
        Err(e) => return McpResponse::error(id, -32603, e.to_string()),
    };

    if !board.has_column(to) {
        return McpResponse::error(id, -32602, format!("Column not found: {to}"));
    }

    let resolved = match board.resolve_card_id(card_id_str) {
        Some(id) => id,
        None => return McpResponse::error(id, -32602, format!("Card not found: {card_id_str}")),
    };

    let next_order = board.next_order(to);
    let card = board.find_card_mut(&resolved).unwrap();
    card.column = to.into();
    card.order = next_order;
    card.updated_at = chrono::Utc::now();
    let result = serde_json::to_string_pretty(card).unwrap();

    if let Err(e) = store.save_board(&board) {
        return McpResponse::error(id, -32603, e.to_string());
    }

    McpResponse::success(id, text_content(&result))
}

fn tool_archive_card(
    id: serde_json::Value,
    args: &serde_json::Value,
    store: &SharedStore,
) -> McpResponse {
    let card_id_str = match args["id"].as_str() {
        Some(s) => s,
        None => return McpResponse::error(id, -32602, "id is required"),
    };

    let store = store.lock().unwrap();
    let config = match store.load_config() {
        Ok(c) => c,
        Err(e) => return McpResponse::error(id, -32603, e.to_string()),
    };
    let mut board = match store.load_board(&config.default_board) {
        Ok(b) => b,
        Err(e) => return McpResponse::error(id, -32603, e.to_string()),
    };

    let resolved = match board.resolve_card_id(card_id_str) {
        Some(id) => id,
        None => return McpResponse::error(id, -32602, format!("Card not found: {card_id_str}")),
    };

    let card = board.find_card_mut(&resolved).unwrap();
    card.archived = true;
    card.updated_at = chrono::Utc::now();
    let result = serde_json::to_string_pretty(card).unwrap();

    if let Err(e) = store.save_board(&board) {
        return McpResponse::error(id, -32603, e.to_string());
    }

    McpResponse::success(id, text_content(&result))
}

fn tool_delete_card(
    id: serde_json::Value,
    args: &serde_json::Value,
    store: &SharedStore,
) -> McpResponse {
    let card_id_str = match args["id"].as_str() {
        Some(s) => s,
        None => return McpResponse::error(id, -32602, "id is required"),
    };

    let store = store.lock().unwrap();
    let config = match store.load_config() {
        Ok(c) => c,
        Err(e) => return McpResponse::error(id, -32603, e.to_string()),
    };
    let mut board = match store.load_board(&config.default_board) {
        Ok(b) => b,
        Err(e) => return McpResponse::error(id, -32603, e.to_string()),
    };

    let resolved = match board.resolve_card_id(card_id_str) {
        Some(id) => id,
        None => return McpResponse::error(id, -32602, format!("Card not found: {card_id_str}")),
    };

    let title = board
        .find_card(&resolved)
        .map(|c| c.title.clone())
        .unwrap_or_default();
    board.cards.retain(|c| c.id != resolved);

    if let Err(e) = store.save_board(&board) {
        return McpResponse::error(id, -32603, e.to_string());
    }

    let result = serde_json::json!({"deleted": resolved, "title": title});
    McpResponse::success(id, text_content(&result.to_string()))
}
