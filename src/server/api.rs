use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use axum::Router;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::Json;
use axum::routing::{delete, get, post, put};
use serde::{Deserialize, Serialize};
use tower_http::cors::CorsLayer;

use crate::error::KukError;
use crate::model::{Board, Card, Column};
use crate::storage::Store;

use super::mcp;

type SharedStore = Arc<Mutex<Store>>;

#[derive(Debug, Serialize)]
struct ApiError {
    error: String,
}

impl ApiError {
    fn new(msg: impl Into<String>) -> (StatusCode, Json<ApiError>) {
        (
            StatusCode::BAD_REQUEST,
            Json(ApiError { error: msg.into() }),
        )
    }

    fn not_found(msg: impl Into<String>) -> (StatusCode, Json<ApiError>) {
        (StatusCode::NOT_FOUND, Json(ApiError { error: msg.into() }))
    }

    fn internal(msg: impl Into<String>) -> (StatusCode, Json<ApiError>) {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError { error: msg.into() }),
        )
    }
}

type ApiResult<T> = Result<Json<T>, (StatusCode, Json<ApiError>)>;

pub async fn serve(repo_root: PathBuf, port: u16, enable_mcp: bool) -> crate::error::Result<()> {
    let store = Store::new(&repo_root);
    if !store.is_initialized() {
        return Err(KukError::NotInitialized);
    }

    let shared = Arc::new(Mutex::new(store));

    let mut routes = Router::new()
        .route("/v1/boards", get(list_boards))
        .route("/v1/boards/{name}", get(get_board))
        .route("/v1/boards", post(create_board))
        .route("/v1/cards", post(add_card))
        .route("/v1/cards/{id}/move", put(move_card))
        .route("/v1/cards/{id}/archive", put(archive_card))
        .route("/v1/cards/{id}/label", put(label_card))
        .route("/v1/cards/{id}/assign", put(assign_card))
        .route("/v1/cards/{id}", delete(delete_card))
        .route("/health", get(health));

    if enable_mcp {
        routes = routes.route("/mcp", post(mcp::mcp_handler));
    }

    let app = routes.layer(CorsLayer::permissive()).with_state(shared);

    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    println!("kuk server listening on http://{addr}");
    if enable_mcp {
        println!("MCP endpoint: http://{addr}/mcp");
    }

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .map_err(|e| KukError::Other(format!("Bind failed: {e}")))?;

    axum::serve(listener, app)
        .await
        .map_err(|e| KukError::Other(format!("Server error: {e}")))?;

    Ok(())
}

// --- Handlers ---

async fn health() -> Json<serde_json::Value> {
    Json(serde_json::json!({"status": "ok", "version": env!("CARGO_PKG_VERSION")}))
}

async fn list_boards(State(store): State<SharedStore>) -> ApiResult<Vec<String>> {
    let store = store.lock().unwrap();
    store
        .list_boards()
        .map(Json)
        .map_err(|e| ApiError::internal(e.to_string()))
}

async fn get_board(State(store): State<SharedStore>, Path(name): Path<String>) -> ApiResult<Board> {
    let store = store.lock().unwrap();
    store.load_board(&name).map(Json).map_err(|e| match e {
        KukError::BoardNotFound(_) => ApiError::not_found(e.to_string()),
        _ => ApiError::internal(e.to_string()),
    })
}

#[derive(Deserialize)]
struct CreateBoardReq {
    name: String,
    #[serde(default = "default_columns")]
    columns: Vec<Column>,
}

fn default_columns() -> Vec<Column> {
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
    ]
}

async fn create_board(
    State(store): State<SharedStore>,
    Json(req): Json<CreateBoardReq>,
) -> ApiResult<serde_json::Value> {
    let store = store.lock().unwrap();
    store
        .create_board(&req.name, req.columns)
        .map(|_| Json(serde_json::json!({"created": req.name})))
        .map_err(|e| ApiError::new(e.to_string()))
}

#[derive(Deserialize)]
struct AddCardReq {
    title: String,
    #[serde(default = "default_column")]
    column: String,
    #[serde(default)]
    labels: Vec<String>,
    #[serde(default)]
    assignee: Option<String>,
    #[serde(default = "default_board_name")]
    board: String,
}

fn default_column() -> String {
    "todo".into()
}
fn default_board_name() -> String {
    "default".into()
}

async fn add_card(
    State(store): State<SharedStore>,
    Json(req): Json<AddCardReq>,
) -> ApiResult<Card> {
    let store = store.lock().unwrap();
    let mut board = store
        .load_board(&req.board)
        .map_err(|e| ApiError::not_found(e.to_string()))?;

    if !board.has_column(&req.column) {
        return Err(ApiError::new(format!("Column not found: {}", req.column)));
    }

    let mut card = Card::new(&req.title, &req.column);
    card.order = board.next_order(&req.column);
    card.labels = req.labels;
    card.assignee = req.assignee;

    let result = card.clone();
    board.cards.push(card);
    store
        .save_board(&board)
        .map_err(|e| ApiError::internal(e.to_string()))?;

    Ok(Json(result))
}

#[derive(Deserialize)]
struct MoveCardReq {
    to: String,
    #[serde(default = "default_board_name")]
    board: String,
}

async fn move_card(
    State(store): State<SharedStore>,
    Path(id): Path<String>,
    Json(req): Json<MoveCardReq>,
) -> ApiResult<Card> {
    let store = store.lock().unwrap();
    let mut board = store
        .load_board(&req.board)
        .map_err(|e| ApiError::not_found(e.to_string()))?;

    if !board.has_column(&req.to) {
        return Err(ApiError::new(format!("Column not found: {}", req.to)));
    }

    let card_id = board
        .resolve_card_id(&id)
        .ok_or_else(|| ApiError::not_found(format!("Card not found: {id}")))?;

    let next_order = board.next_order(&req.to);
    let card = board
        .find_card_mut(&card_id)
        .ok_or_else(|| ApiError::not_found(format!("Card not found: {id}")))?;

    card.column = req.to;
    card.order = next_order;
    card.updated_at = chrono::Utc::now();
    let result = card.clone();

    store
        .save_board(&board)
        .map_err(|e| ApiError::internal(e.to_string()))?;

    Ok(Json(result))
}

async fn archive_card(State(store): State<SharedStore>, Path(id): Path<String>) -> ApiResult<Card> {
    let store = store.lock().unwrap();
    let config = store
        .load_config()
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let mut board = store
        .load_board(&config.default_board)
        .map_err(|e| ApiError::internal(e.to_string()))?;

    let card_id = board
        .resolve_card_id(&id)
        .ok_or_else(|| ApiError::not_found(format!("Card not found: {id}")))?;

    let card = board
        .find_card_mut(&card_id)
        .ok_or_else(|| ApiError::not_found(format!("Card not found: {id}")))?;

    card.archived = true;
    card.updated_at = chrono::Utc::now();
    let result = card.clone();

    store
        .save_board(&board)
        .map_err(|e| ApiError::internal(e.to_string()))?;

    Ok(Json(result))
}

#[derive(Deserialize)]
struct LabelReq {
    action: String, // "add" or "remove"
    tag: String,
}

async fn label_card(
    State(store): State<SharedStore>,
    Path(id): Path<String>,
    Json(req): Json<LabelReq>,
) -> ApiResult<Card> {
    let store = store.lock().unwrap();
    let config = store
        .load_config()
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let mut board = store
        .load_board(&config.default_board)
        .map_err(|e| ApiError::internal(e.to_string()))?;

    let card_id = board
        .resolve_card_id(&id)
        .ok_or_else(|| ApiError::not_found(format!("Card not found: {id}")))?;

    let card = board
        .find_card_mut(&card_id)
        .ok_or_else(|| ApiError::not_found(format!("Card not found: {id}")))?;

    match req.action.as_str() {
        "add" => {
            if !card.labels.contains(&req.tag) {
                card.labels.push(req.tag);
            }
        }
        "remove" => {
            card.labels.retain(|l| l != &req.tag);
        }
        _ => return Err(ApiError::new("action must be 'add' or 'remove'")),
    }

    card.updated_at = chrono::Utc::now();
    let result = card.clone();

    store
        .save_board(&board)
        .map_err(|e| ApiError::internal(e.to_string()))?;

    Ok(Json(result))
}

#[derive(Deserialize)]
struct AssignReq {
    user: String,
}

async fn assign_card(
    State(store): State<SharedStore>,
    Path(id): Path<String>,
    Json(req): Json<AssignReq>,
) -> ApiResult<Card> {
    let store = store.lock().unwrap();
    let config = store
        .load_config()
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let mut board = store
        .load_board(&config.default_board)
        .map_err(|e| ApiError::internal(e.to_string()))?;

    let card_id = board
        .resolve_card_id(&id)
        .ok_or_else(|| ApiError::not_found(format!("Card not found: {id}")))?;

    let card = board
        .find_card_mut(&card_id)
        .ok_or_else(|| ApiError::not_found(format!("Card not found: {id}")))?;

    card.assignee = Some(req.user);
    card.updated_at = chrono::Utc::now();
    let result = card.clone();

    store
        .save_board(&board)
        .map_err(|e| ApiError::internal(e.to_string()))?;

    Ok(Json(result))
}

async fn delete_card(
    State(store): State<SharedStore>,
    Path(id): Path<String>,
) -> ApiResult<serde_json::Value> {
    let store = store.lock().unwrap();
    let config = store
        .load_config()
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let mut board = store
        .load_board(&config.default_board)
        .map_err(|e| ApiError::internal(e.to_string()))?;

    let card_id = board
        .resolve_card_id(&id)
        .ok_or_else(|| ApiError::not_found(format!("Card not found: {id}")))?;

    let title = board
        .find_card(&card_id)
        .map(|c| c.title.clone())
        .unwrap_or_default();

    board.cards.retain(|c| c.id != card_id);

    store
        .save_board(&board)
        .map_err(|e| ApiError::internal(e.to_string()))?;

    Ok(Json(
        serde_json::json!({"deleted": card_id, "title": title}),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{self, Request};
    use tempfile::TempDir;
    use tower::ServiceExt;

    fn test_app() -> (TempDir, Router) {
        let dir = TempDir::new().unwrap();
        let store = Store::new(dir.path());
        store.init().unwrap();

        let shared = Arc::new(Mutex::new(Store::new(dir.path())));
        let app = Router::new()
            .route("/v1/boards", get(list_boards))
            .route("/v1/boards/{name}", get(get_board))
            .route("/v1/boards", post(create_board))
            .route("/v1/cards", post(add_card))
            .route("/v1/cards/{id}/move", put(move_card))
            .route("/v1/cards/{id}/archive", put(archive_card))
            .route("/v1/cards/{id}/label", put(label_card))
            .route("/v1/cards/{id}/assign", put(assign_card))
            .route("/v1/cards/{id}", delete(delete_card))
            .route("/health", get(health))
            .route("/mcp", post(mcp::mcp_handler))
            .with_state(shared);

        (dir, app)
    }

    async fn body_json(body: Body) -> serde_json::Value {
        let bytes = axum::body::to_bytes(body, usize::MAX).await.unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }

    #[tokio::test]
    async fn health_check() {
        let (_dir, app) = test_app();
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let json = body_json(resp.into_body()).await;
        assert_eq!(json["status"], "ok");
    }

    #[tokio::test]
    async fn list_boards_returns_default() {
        let (_dir, app) = test_app();
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/v1/boards")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let json = body_json(resp.into_body()).await;
        let boards: Vec<String> = serde_json::from_value(json).unwrap();
        assert_eq!(boards, vec!["default"]);
    }

    #[tokio::test]
    async fn get_board_default() {
        let (_dir, app) = test_app();
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/v1/boards/default")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let json = body_json(resp.into_body()).await;
        assert_eq!(json["name"], "default");
        assert_eq!(json["columns"].as_array().unwrap().len(), 3);
    }

    #[tokio::test]
    async fn get_board_not_found() {
        let (_dir, app) = test_app();
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/v1/boards/nonexistent")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn add_and_get_card() {
        let (_dir, app) = test_app();

        // Add card
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(http::Method::POST)
                    .uri("/v1/cards")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({"title": "Test card"}).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let card = body_json(resp.into_body()).await;
        assert_eq!(card["title"], "Test card");
        assert_eq!(card["column"], "todo");
        let card_id = card["id"].as_str().unwrap().to_string();

        // Verify via board
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/v1/boards/default")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let board = body_json(resp.into_body()).await;
        assert_eq!(board["cards"].as_array().unwrap().len(), 1);
        assert_eq!(board["cards"][0]["id"], card_id);
    }

    #[tokio::test]
    async fn add_card_to_invalid_column() {
        let (_dir, app) = test_app();
        let resp = app
            .oneshot(
                Request::builder()
                    .method(http::Method::POST)
                    .uri("/v1/cards")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({"title": "Bad", "column": "nope"}).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn move_card_via_api() {
        let (_dir, app) = test_app();

        // Add card
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(http::Method::POST)
                    .uri("/v1/cards")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({"title": "Move me"}).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        let card = body_json(resp.into_body()).await;
        let card_id = card["id"].as_str().unwrap();

        // Move card
        let resp = app
            .oneshot(
                Request::builder()
                    .method(http::Method::PUT)
                    .uri(format!("/v1/cards/{card_id}/move"))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::json!({"to": "doing"}).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let moved = body_json(resp.into_body()).await;
        assert_eq!(moved["column"], "doing");
    }

    #[tokio::test]
    async fn delete_card_via_api() {
        let (_dir, app) = test_app();

        // Add card
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(http::Method::POST)
                    .uri("/v1/cards")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({"title": "Delete me"}).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        let card = body_json(resp.into_body()).await;
        let card_id = card["id"].as_str().unwrap().to_string();

        // Delete card
        let resp = app
            .oneshot(
                Request::builder()
                    .method(http::Method::DELETE)
                    .uri(format!("/v1/cards/{card_id}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let result = body_json(resp.into_body()).await;
        assert_eq!(result["deleted"], card_id);
    }

    #[tokio::test]
    async fn archive_card_via_api() {
        let (_dir, app) = test_app();

        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(http::Method::POST)
                    .uri("/v1/cards")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({"title": "Archive me"}).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        let card = body_json(resp.into_body()).await;
        let card_id = card["id"].as_str().unwrap().to_string();

        let resp = app
            .oneshot(
                Request::builder()
                    .method(http::Method::PUT)
                    .uri(format!("/v1/cards/{card_id}/archive"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let result = body_json(resp.into_body()).await;
        assert_eq!(result["archived"], true);
    }

    #[tokio::test]
    async fn label_card_via_api() {
        let (_dir, app) = test_app();

        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(http::Method::POST)
                    .uri("/v1/cards")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({"title": "Label me"}).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        let card = body_json(resp.into_body()).await;
        let card_id = card["id"].as_str().unwrap().to_string();

        let resp = app
            .oneshot(
                Request::builder()
                    .method(http::Method::PUT)
                    .uri(format!("/v1/cards/{card_id}/label"))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({"action": "add", "tag": "bug"}).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let result = body_json(resp.into_body()).await;
        assert!(
            result["labels"]
                .as_array()
                .unwrap()
                .contains(&serde_json::json!("bug"))
        );
    }

    #[tokio::test]
    async fn assign_card_via_api() {
        let (_dir, app) = test_app();

        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(http::Method::POST)
                    .uri("/v1/cards")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({"title": "Assign me"}).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        let card = body_json(resp.into_body()).await;
        let card_id = card["id"].as_str().unwrap().to_string();

        let resp = app
            .oneshot(
                Request::builder()
                    .method(http::Method::PUT)
                    .uri(format!("/v1/cards/{card_id}/assign"))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({"user": "leslie"}).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let result = body_json(resp.into_body()).await;
        assert_eq!(result["assignee"], "leslie");
    }

    #[tokio::test]
    async fn create_board_via_api() {
        let (_dir, app) = test_app();

        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(http::Method::POST)
                    .uri("/v1/boards")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({"name": "sprint-1"}).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/v1/boards")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let boards = body_json(resp.into_body()).await;
        let arr: Vec<String> = serde_json::from_value(boards).unwrap();
        assert!(arr.contains(&"sprint-1".to_string()));
    }

    #[tokio::test]
    async fn mcp_tools_list() {
        let (_dir, app) = test_app();
        let resp = app
            .oneshot(
                Request::builder()
                    .method(http::Method::POST)
                    .uri("/mcp")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({
                            "jsonrpc": "2.0",
                            "id": 1,
                            "method": "tools/list"
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let json = body_json(resp.into_body()).await;
        assert!(json["result"]["tools"].is_array());
    }

    #[tokio::test]
    async fn mcp_add_card() {
        let (_dir, app) = test_app();
        let resp = app
            .oneshot(
                Request::builder()
                    .method(http::Method::POST)
                    .uri("/mcp")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({
                            "jsonrpc": "2.0",
                            "id": 2,
                            "method": "tools/call",
                            "params": {
                                "name": "kuk_add_card",
                                "arguments": {
                                    "title": "MCP card"
                                }
                            }
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let json = body_json(resp.into_body()).await;
        let content = &json["result"]["content"][0]["text"];
        let card: serde_json::Value = serde_json::from_str(content.as_str().unwrap()).unwrap();
        assert_eq!(card["title"], "MCP card");
    }

    #[tokio::test]
    async fn mcp_list_cards() {
        let (_dir, app) = test_app();

        // Add a card first
        app.clone()
            .oneshot(
                Request::builder()
                    .method(http::Method::POST)
                    .uri("/v1/cards")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({"title": "For MCP list"}).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        let resp = app
            .oneshot(
                Request::builder()
                    .method(http::Method::POST)
                    .uri("/mcp")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({
                            "jsonrpc": "2.0",
                            "id": 3,
                            "method": "tools/call",
                            "params": {
                                "name": "kuk_list_cards",
                                "arguments": {}
                            }
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let json = body_json(resp.into_body()).await;
        let content = &json["result"]["content"][0]["text"];
        let board: serde_json::Value = serde_json::from_str(content.as_str().unwrap()).unwrap();
        assert_eq!(board["cards"].as_array().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn mcp_move_card() {
        let (_dir, app) = test_app();

        // Add card via REST
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(http::Method::POST)
                    .uri("/v1/cards")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({"title": "MCP move"}).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        let card = body_json(resp.into_body()).await;
        let card_id = card["id"].as_str().unwrap();

        // Move via MCP
        let resp = app
            .oneshot(
                Request::builder()
                    .method(http::Method::POST)
                    .uri("/mcp")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({
                            "jsonrpc": "2.0",
                            "id": 4,
                            "method": "tools/call",
                            "params": {
                                "name": "kuk_move_card",
                                "arguments": {
                                    "id": card_id,
                                    "to": "doing"
                                }
                            }
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let json = body_json(resp.into_body()).await;
        let content = &json["result"]["content"][0]["text"];
        let moved: serde_json::Value = serde_json::from_str(content.as_str().unwrap()).unwrap();
        assert_eq!(moved["column"], "doing");
    }
}
