:::    ::: :::    ::: :::    ::: 
:+:   :+:  :+:    :+: :+:   :+:  
+:+  +:+   +:+    +:+ +:+  +:+   
+#++:++    +#+    +:+ +#++:++    
+#+  +#+   +#+    +#+ +#+  +#+   
#+#   #+#  #+#    #+# #+#   #+#  
###    ###  ########  ###    ### 
kuk = Kanban Under Kontrol
(also yes we know what it sounds like, grow up)

# kuk

**Kanban that ships with your code.**

A single binary that turns every git repo into a fully functional Kanban board stored as plain JSON inside `.kuk/`. Zero config for solo devs. Full REST + MCP server when you run `kuk serve`. CLI, TUI, and API — all in one. Paired with `kuk-pm` for git-native project management: auto-branching from cards, commit tracking, and cross-repo views.

```
$ kuk init
Initialized kuk board in .kuk

$ kuk add "Build the thing" --label mvp
Added: Build the thing → todo

$ kuk add "Write tests" --label tdd
Added: Write tests → todo

$ kuk move 1 --to doing
Moved: Build the thing → doing

$ kuk list
── TODO (1)──
  1. Write tests [tdd]

── DOING (1)──
  1. Build the thing [mvp]

── DONE (0)──
```

---

## Table of Contents

- [Why kuk?](#why-kuk)
- [Installation](#installation)
- [Quick Start](#quick-start)
- [CLI Reference](#cli-reference)
- [TUI](#tui)
- [REST API](#rest-api)
- [MCP (Model Context Protocol)](#mcp-model-context-protocol)
- [Data Model](#data-model)
- [Storage Layout](#storage-layout)
- [Configuration](#configuration)
- [Project Architecture](#project-architecture)
- [Testing](#testing)
- [Building from Source](#building-from-source)
- [kuk-pm: Project Manager](#kuk-pm-project-manager)
- [License](#license)

---

## Why kuk?

| Feature | kuk | kanban-tui | taskell | clikan |
|---------|-----|-----------|---------|--------|
| Repo-scoped + global index | Yes | No | No | No |
| Pure JSON storage (git-diff friendly) | Yes | No | No | No |
| Built-in MCP server (AI agents) | Yes | No | No | No |
| Single binary (CLI + TUI + server) | Yes | No | No | No |
| REST API | Yes | No | No | No |
| Local-first, zero config | Yes | Partial | Yes | Yes |
| Vim keybindings in TUI | Yes | Yes | Yes | No |

kuk is designed from day one for:
- **Solo devs** who want a board per repo with zero cloud dependency
- **AI power users** who want agents (Claude, Cursor, etc.) to manage cards via MCP
- **Scriptable workflows** where every action is a CLI subcommand with `--json` output

---

## Installation

### Pre-built Binaries

Download the latest release for your platform from the [Releases](https://github.com/leslierussell/kuk/releases) page.

```bash
# macOS (Apple Silicon)
curl -L https://github.com/leslierussell/kuk/releases/latest/download/kuk-aarch64-apple-darwin -o kuk
chmod +x kuk
sudo mv kuk /usr/local/bin/

# macOS (Intel)
curl -L https://github.com/leslierussell/kuk/releases/latest/download/kuk-x86_64-apple-darwin -o kuk
chmod +x kuk
sudo mv kuk /usr/local/bin/

# Linux (x86_64)
curl -L https://github.com/leslierussell/kuk/releases/latest/download/kuk-x86_64-unknown-linux-gnu -o kuk
chmod +x kuk
sudo mv kuk /usr/local/bin/
```

### From Source (cargo install)

```bash
git clone https://github.com/leslierussell/kuk.git
cd kuk

# Install both binaries globally (~/.cargo/bin/)
cargo install --path .          # installs kuk
cargo install --path kuk-pm     # installs kuk-pm
```

Or build manually without installing:

```bash
cargo build --release
# Binaries at:
#   target/release/kuk      (2.7 MB stripped)
#   target/release/kuk-pm   (2.9 MB stripped)
```

Make sure `~/.cargo/bin` is in your `$PATH`:

```bash
export PATH="$HOME/.cargo/bin:$PATH"   # add to ~/.zshrc or ~/.bashrc
```

### Verify Installation

```bash
kuk version
# kuk 0.1.0

kuk-pm version
# kuk-pm 0.1.0

kuk doctor
# kuk doctor
# ──────────
#   [!!] .kuk/ not found. Run `kuk init`.
```

---

## Quick Start

```bash
# 1. Initialize in any repo
cd my-project
kuk init

# 2. Add some cards
kuk add "Set up CI pipeline" --label infra --assignee leslie
kuk add "Fix login bug" --label bug --label urgent
kuk add "Write API docs" --to doing

# 3. View the board
kuk list

# 4. Move cards through the workflow
kuk move 1 --to doing
kuk move 2 --to done

# 5. Launch the TUI for a visual board
kuk tui

# 6. Start the API server (with MCP for AI agents)
kuk serve --port 8080 --mcp

# 7. Set up project management (git integration)
kuk-pm init
kuk-pm branch 1          # Creates feature/set-up-ci-pipeline branch
kuk-pm doctor             # Health check
```

---

## CLI Reference

All commands support these global flags:

| Flag | Description |
|------|-------------|
| `--json` | Output as JSON (machine-readable) |
| `--quiet` | Suppress non-essential output |
| `--repo <PATH>` | Target a different repo (defaults to current directory) |

### `kuk init`

Initialize a new kuk board in the current directory.

```bash
kuk init                          # Creates .kuk/ with default board
kuk init --board-name sprint-1    # Custom initial board name
```

Creates:
- `.kuk/config.json` — per-repo configuration
- `.kuk/boards/default.json` — the default board
- Registers the project in `~/.kuk/index.json` (global index)

Running `kuk init` twice returns an error (idempotent guard).

### `kuk add <title>`

Add a new card to the board.

```bash
kuk add "Implement auth"
kuk add "Fix bug #42" --to doing
kuk add "Deploy v2" --label release --label urgent --assignee leslie
kuk add "Quick task" --json    # Returns the card as JSON
```

| Flag | Default | Description |
|------|---------|-------------|
| `--to <column>` | `todo` | Target column |
| `--label <tag>` | (none) | Add labels (repeatable) |
| `--assignee <user>` | (none) | Assign a user |

Cards are assigned a [ULID](https://github.com/ulid/spec) as their ID and placed at the bottom of the target column.

### `kuk list`

Display the active board. If no `--board` is specified, uses the currently active board (set via `kuk board switch`).

```bash
kuk list                    # Active board, human-readable
kuk list --board sprint-1   # Specific board (overrides active)
kuk list --json             # Full board as JSON
```

Output format (human-readable):
```
── TODO (2)──
  1. Implement auth
  2. Fix bug #42 [bug] @leslie

── DOING (1)──
  1. Deploy v2 [release, urgent]

── DONE (0)──
```

Numbers are 1-based display indices — use them with `move`, `archive`, `delete`, etc.

### `kuk move <id> --to <column>`

Move a card to a different column.

```bash
kuk move 1 --to doing                       # By display number
kuk move 01HXYZ1234567890ABCDEFGHIJ --to done  # By ULID
```

### `kuk hoist <id>`

Move a card to the top of its current column.

```bash
kuk hoist 2    # Card #2 is now first in its column
```

### `kuk demote <id>`

Move a card to the bottom of its current column.

```bash
kuk demote 1   # Card #1 is now last in its column
```

### `kuk archive <id>`

Archive a card. Archived cards are hidden from `list` but remain in the JSON file.

```bash
kuk archive 1
```

### `kuk delete <id>`

Permanently delete a card from the board.

```bash
kuk delete 1
kuk delete 1 --json    # Returns {"deleted": "<id>", "title": "<title>"}
```

### `kuk label <id> <add|remove> <tag>`

Add or remove labels from a card.

```bash
kuk label 1 add bug
kuk label 1 add urgent
kuk label 1 remove bug
```

### `kuk assign <id> <user>`

Assign a user to a card.

```bash
kuk assign 1 leslie
```

### `kuk board <subcommand>`

Manage multiple boards. Works like `git branch` — switching boards persists across all subsequent commands until you switch again.

```bash
kuk board list                # List all boards (* marks active)
kuk board create sprint-1     # Create a new board
kuk board switch sprint-1     # Switch the active board
```

**Listing boards** shows the active board with a `*` prefix, just like `git branch`:

```
$ kuk board list
  default
* sprint-1
```

**Switching boards** persists the active board in `.kuk/config.json`. All commands (`add`, `list`, `move`, etc.) operate on the active board by default:

```bash
$ kuk board switch sprint-1
Switched to board: sprint-1

$ kuk add "Sprint task"
Added: Sprint task → todo       # Added to sprint-1, not default

$ kuk list                      # Shows sprint-1
$ kuk list --board default      # Explicitly target a different board
```

Switching to a nonexistent board returns an error.

New boards are created with default columns: `todo`, `doing`, `done`.

### `kuk projects`

List all kuk-enabled repos on the machine.

```bash
kuk projects
#   my-project → /home/leslie/repos/my-project
#   api-server → /home/leslie/repos/api-server

kuk projects --json    # Full index as JSON
```

The global index lives at `~/.kuk/index.json` and is updated automatically on `kuk init`.

### `kuk doctor`

Run a health check on the current repo's kuk installation.

```bash
kuk doctor
# kuk doctor
# ──────────
#   [OK] .kuk/ directory found
#   [OK] config.json (v0.1.0)
#   [OK] 1 board(s): default
#        └─ default: 3 active, 0 archived
#   [OK] global index: 5 projects
#
# All checks passed.
```

### `kuk serve`

Start the REST API and optional MCP server.

```bash
kuk serve                          # localhost:8080, REST only
kuk serve --port 3000              # Custom port
kuk serve --port 8080 --mcp       # REST + MCP endpoint
```

| Flag | Default | Description |
|------|---------|-------------|
| `--port <u16>` | `8080` | Port to listen on |
| `--mcp` | `false` | Enable MCP endpoint at `/mcp` |

### `kuk tui`

Launch the interactive terminal UI. See [TUI](#tui) section below.

### `kuk version`

Print the version.

```bash
kuk version
# kuk 0.1.0
```

---

## TUI

Launch with `kuk tui`. The TUI presents a visual Kanban board with full vim-style navigation.

### Modes

| Mode | Description | Entered via |
|------|-------------|-------------|
| **NORMAL** | Default mode — navigate and act | (default) |
| **INSERT** | Type a card title to add | `a` |
| **SEARCH** | Filter cards by title | `/` |
| **BOARDS** | Switch between boards | `b` |
| **HELP** | Show keybinding reference | `?` |
| **CONFIRM** | Confirm destructive action | `d` (delete) |

### Keybindings

#### Navigation (NORMAL mode)

| Key | Action |
|-----|--------|
| `j` / `Down` | Move cursor down |
| `k` / `Up` | Move cursor up |
| `h` / `Left` | Switch to previous column |
| `l` / `Right` | Switch to next column |
| `gg` | Jump to top of column |
| `G` | Jump to bottom of column |

#### Card Actions (NORMAL mode)

| Key | Action |
|-----|--------|
| `a` | Add new card to current column |
| `d` | Delete card (with confirmation) |
| `x` | Archive card |
| `L` or `>` | Move card to next column (right) |
| `H` or `<` | Move card to previous column (left) |
| `K` | Hoist card to top of column |
| `J` | Demote card to bottom of column |

#### Other (NORMAL mode)

| Key | Action |
|-----|--------|
| `b` | Switch board (picker overlay) |
| `/` | Search cards by title |
| `r` | Refresh board from disk |
| `?` | Toggle help overlay |
| `q` | Quit |
| `Ctrl+C` | Quit (always works, any mode) |

#### BOARDS mode

| Key | Action |
|-----|--------|
| `j` / `Down` | Move selection down |
| `k` / `Up` | Move selection up |
| `Enter` | Switch to selected board |
| `Esc` / `q` | Cancel |

The active board is shown with a `*` prefix. Switching persists to `.kuk/config.json`.

#### INSERT mode

| Key | Action |
|-----|--------|
| Characters | Type card title |
| `Backspace` | Delete character |
| `Enter` | Save card |
| `Esc` | Cancel |

#### SEARCH mode

| Key | Action |
|-----|--------|
| Characters | Type search query |
| `Backspace` | Delete character |
| `Enter` | Apply filter, return to NORMAL |
| `Esc` | Clear search, return to NORMAL |

#### CONFIRM mode

| Key | Action |
|-----|--------|
| `y` / `Y` | Confirm action |
| Any other key | Cancel |

### TUI Layout

```
┌─────────────────────────────────────────────────────────┐
│ kuk  │  default  │  5 cards                             │  <- Title bar
├──────────────┬──────────────┬───────────────────────────┤
│ TODO (2)     │ DOING (2)    │ DONE (1)                  │
│              │              │                           │
│ Fix login    │ Build API    │ Set up repo               │
│ Write docs   │ Deploy v2    │                           │
│              │              │                           │
├──────────────┴──────────────┴───────────────────────────┤
│ NORMAL │ ? for help                                     │  <- Status bar
└─────────────────────────────────────────────────────────┘
```

The selected card is highlighted in cyan. The active column border is cyan.

---

## REST API

Start the server with `kuk serve --port 8080`. All endpoints accept and return JSON.

### Endpoints

#### Health

```
GET /health
```

Response:
```json
{"status": "ok", "version": "0.1.0"}
```

#### Boards

```
GET    /v1/boards          List all board names
GET    /v1/boards/{name}   Get a board with all its cards
POST   /v1/boards          Create a new board
```

**List boards:**
```bash
curl http://localhost:8080/v1/boards
# ["default", "sprint-1"]
```

**Get board:**
```bash
curl http://localhost:8080/v1/boards/default
```
```json
{
  "name": "default",
  "columns": [
    {"name": "todo"},
    {"name": "doing"},
    {"name": "done"}
  ],
  "cards": [...]
}
```

**Create board:**
```bash
curl -X POST http://localhost:8080/v1/boards \
  -H "content-type: application/json" \
  -d '{"name": "sprint-1"}'
# {"created": "sprint-1"}
```

Optionally pass `"columns"` array; defaults to `todo`/`doing`/`done`.

#### Cards

```
POST   /v1/cards                  Add a card
PUT    /v1/cards/{id}/move        Move a card
PUT    /v1/cards/{id}/archive     Archive a card
PUT    /v1/cards/{id}/label       Add/remove label
PUT    /v1/cards/{id}/assign      Assign user
DELETE /v1/cards/{id}             Delete a card
```

**Add card:**
```bash
curl -X POST http://localhost:8080/v1/cards \
  -H "content-type: application/json" \
  -d '{"title": "New task", "column": "todo", "labels": ["bug"], "assignee": "leslie"}'
```
```json
{
  "id": "01HXYZ1234567890ABCDEFGHIJ",
  "title": "New task",
  "column": "todo",
  "order": 0,
  "labels": ["bug"],
  "assignee": "leslie",
  "created_at": "2026-02-25T12:00:00Z",
  "updated_at": "2026-02-25T12:00:00Z",
  "metadata": {},
  "archived": false
}
```

**Move card:**
```bash
curl -X PUT http://localhost:8080/v1/cards/01HXYZ.../move \
  -H "content-type: application/json" \
  -d '{"to": "doing"}'
```

**Archive card:**
```bash
curl -X PUT http://localhost:8080/v1/cards/01HXYZ.../archive
```

**Label card:**
```bash
curl -X PUT http://localhost:8080/v1/cards/01HXYZ.../label \
  -H "content-type: application/json" \
  -d '{"action": "add", "tag": "urgent"}'
```

**Assign card:**
```bash
curl -X PUT http://localhost:8080/v1/cards/01HXYZ.../assign \
  -H "content-type: application/json" \
  -d '{"user": "leslie"}'
```

**Delete card:**
```bash
curl -X DELETE http://localhost:8080/v1/cards/01HXYZ...
# {"deleted": "01HXYZ...", "title": "New task"}
```

#### Error Responses

All errors return a JSON object with an `error` field:

```json
{"error": "Card not found: 99"}
```

HTTP status codes:
- `400` — Bad request (invalid column, invalid action, etc.)
- `404` — Not found (board, card)
- `500` — Internal server error

CORS is enabled (permissive) on all endpoints.

---

## MCP (Model Context Protocol)

kuk implements MCP so AI agents (Claude Code, Cursor, GPT, etc.) can manage your board and project directly. Two transports are supported:

| Transport | Command | Use Case |
|-----------|---------|----------|
| **stdio** | `kuk mcp` / `kuk-pm mcp` | Claude Code, local AI agents |
| **HTTP** | `kuk serve --mcp` | Remote agents, web integrations |

### Claude Code Setup (Recommended)

Add a `.mcp.json` to your project root:

```json
{
  "mcpServers": {
    "kuk": {
      "type": "stdio",
      "command": "kuk",
      "args": ["mcp"],
      "env": {}
    },
    "kuk-pm": {
      "type": "stdio",
      "command": "kuk-pm",
      "args": ["mcp"],
      "env": {}
    }
  }
}
```

Restart Claude Code. Both servers will appear as available MCP tools. The `kuk` server provides kanban board management; the `kuk-pm` server provides project management, sprints, and analytics.

### kuk MCP Tools (Kanban Board)

| Tool | Description | Required Args |
|------|-------------|---------------|
| `kuk_add_card` | Add a new card | `title` |
| `kuk_list_cards` | List all cards grouped by column | (none) |
| `kuk_move_card` | Move a card to a column | `id`, `to` |
| `kuk_archive_card` | Archive a card (hidden, not deleted) | `id` |
| `kuk_delete_card` | Permanently delete a card | `id` |
| `kuk_list_boards` | List all board names | (none) |
| `kuk_board_info` | Board details with card counts | (none) |

### kuk-pm MCP Tools (Project Management)

| Tool | Description | Required Args |
|------|-------------|---------------|
| `pm_stats` | Card counts, WIP, cycle time, throughput | (none) |
| `pm_velocity` | Cards completed per week with trend | (none) |
| `pm_burndown` | Ideal vs actual burndown for a sprint | (none) |
| `pm_roadmap` | Projected card flow with milestones | (none) |
| `pm_sprint_list` | List all sprints with status | (none) |
| `pm_sprint_create` | Create a new sprint | `name`, `start`, `end` |
| `pm_sprint_start` | Start a planned sprint | `name` |
| `pm_sprint_end` | Close an active sprint | `name` |
| `pm_link` | Link a card to a GitHub issue/PR URL | `card_id`, `url` |
| `pm_release_notes` | Generate release notes from git history | (none) |
| `pm_sync` | Sync board with GitHub issues/PRs | (none) |

### Tool Input Schemas

**kuk_add_card:**
| Field | Type | Required | Default |
|-------|------|----------|---------|
| `title` | string | Yes | — |
| `column` | string | No | `"todo"` |
| `labels` | string[] | No | `[]` |
| `assignee` | string | No | `null` |
| `board` | string | No | `"default"` |

**kuk_list_cards / kuk_board_info / kuk_list_boards:**
| Field | Type | Required | Default |
|-------|------|----------|---------|
| `board` | string | No | `"default"` |

**kuk_move_card:**
| Field | Type | Required | Default |
|-------|------|----------|---------|
| `id` | string | Yes | — |
| `to` | string | Yes | — |
| `board` | string | No | `"default"` |

**kuk_archive_card / kuk_delete_card:**
| Field | Type | Required | Default |
|-------|------|----------|---------|
| `id` | string | Yes | — |
| `board` | string | No | `"default"` |

**pm_velocity:**
| Field | Type | Required | Default |
|-------|------|----------|---------|
| `weeks` | number | No | `4` |

**pm_burndown:**
| Field | Type | Required | Default |
|-------|------|----------|---------|
| `sprint` | string | No | active sprint |

**pm_roadmap:**
| Field | Type | Required | Default |
|-------|------|----------|---------|
| `weeks` | number | No | `12` |

**pm_sprint_create:**
| Field | Type | Required | Default |
|-------|------|----------|---------|
| `name` | string | Yes | — |
| `start` | string | Yes | — |
| `end` | string | Yes | — |

**pm_sprint_start / pm_sprint_end:**
| Field | Type | Required | Default |
|-------|------|----------|---------|
| `name` | string | Yes | — |

**pm_link:**
| Field | Type | Required | Default |
|-------|------|----------|---------|
| `card_id` | string | Yes | — |
| `url` | string | Yes | — |

**pm_release_notes:**
| Field | Type | Required | Default |
|-------|------|----------|---------|
| `since` | string | No | last tag |

**pm_sync:**
| Field | Type | Required | Default |
|-------|------|----------|---------|
| `dry_run` | boolean | No | `false` |

### HTTP Transport

For remote access or web integrations, use the HTTP MCP endpoint:

```bash
kuk serve --port 8080 --mcp
# MCP endpoint: POST http://127.0.0.1:8080/mcp
```

The HTTP endpoint accepts JSON-RPC 2.0 requests:

```bash
# List tools
curl -X POST http://localhost:8080/mcp \
  -H "content-type: application/json" \
  -d '{"jsonrpc": "2.0", "id": 1, "method": "tools/list"}'

# Add a card
curl -X POST http://localhost:8080/mcp \
  -H "content-type: application/json" \
  -d '{
    "jsonrpc": "2.0", "id": 2,
    "method": "tools/call",
    "params": {
      "name": "kuk_add_card",
      "arguments": {"title": "AI-created task", "labels": ["ai-generated"]}
    }
  }'
```

### Response Format

Success:
```json
{
  "jsonrpc": "2.0",
  "id": 2,
  "result": {
    "content": [
      {"type": "text", "text": "{\"id\": \"01HXYZ...\", ...}"}
    ]
  }
}
```

Error:
```json
{
  "jsonrpc": "2.0",
  "id": 2,
  "error": {
    "code": -32602,
    "message": "Card not found: 99"
  }
}
```

---

## Data Model

### Card

```json
{
  "id": "01KJAYWNCMX4YGYW3DC7GGHA4S",
  "title": "Implement authentication",
  "column": "doing",
  "order": 0,
  "description": "OAuth2 + session tokens",
  "assignee": "leslie",
  "labels": ["feature", "auth"],
  "due": "2026-03-01T00:00:00Z",
  "created_at": "2026-02-25T12:00:00Z",
  "updated_at": "2026-02-25T14:30:00Z",
  "metadata": {
    "pr_url": "https://github.com/example/repo/pull/42"
  },
  "archived": false
}
```

| Field | Type | Description |
|-------|------|-------------|
| `id` | string | [ULID](https://github.com/ulid/spec) — 26 chars, time-sortable, unique |
| `title` | string | Card title |
| `column` | string | Current column name |
| `order` | u32 | Sort position within column (0 = top) |
| `description` | string? | Optional long description |
| `assignee` | string? | Optional username |
| `labels` | string[] | Tags/labels |
| `due` | ISO8601? | Optional due date |
| `created_at` | ISO8601 | Creation timestamp |
| `updated_at` | ISO8601 | Last modification timestamp |
| `metadata` | object | Arbitrary key-value pairs (PR URLs, issue links, etc.) |
| `archived` | bool | Hidden from list when true, retained in JSON |

### Board

```json
{
  "name": "default",
  "columns": [
    {"name": "todo"},
    {"name": "doing"},
    {"name": "done", "wip_limit": 10}
  ],
  "cards": [...]
}
```

| Field | Type | Description |
|-------|------|-------------|
| `name` | string | Board identifier |
| `columns` | Column[] | Ordered list of columns |
| `cards` | Card[] | All cards (including archived) |

### Column

| Field | Type | Description |
|-------|------|-------------|
| `name` | string | Column identifier (e.g., `"todo"`) |
| `wip_limit` | u32? | Optional work-in-progress limit |

---

## Storage Layout

kuk stores everything as pretty-printed JSON. Every file is designed to produce clean `git diff` output.

```
~/.kuk/
  index.json              # Global project registry

<your-repo>/
  .kuk/
    config.json           # Per-repo settings
    boards/
      default.json        # Default board
      sprint-1.json       # Additional boards
```

### Global Index (`~/.kuk/index.json`)

```json
{
  "projects": [
    {
      "path": "/home/leslie/repos/my-project",
      "name": "my-project",
      "added_at": "2026-02-25T12:00:00Z"
    }
  ]
}
```

### Repo Config (`.kuk/config.json`)

```json
{
  "version": "0.1.0",
  "default_board": "default"
}
```

### Git Integration

Add `.kuk/` to your repo to share the board with your team, or add it to `.gitignore` for private use:

```bash
# Share the board (recommended)
git add .kuk/
git commit -m "Add kanban board"

# Or keep it private
echo ".kuk/" >> .gitignore
```

Since all data is pretty-printed JSON, `git diff` shows exactly what changed:

```diff
  {
    "title": "Fix login bug",
-   "column": "todo",
+   "column": "doing",
    "order": 0,
```

---

## Configuration

### Per-Repo Config (`.kuk/config.json`)

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `version` | string | `"0.1.0"` | Config schema version |
| `default_board` | string | `"default"` | Active board name |

### Environment

kuk respects:
- `$EDITOR` — used by `kuk edit` (future)
- Current working directory — or override with `--repo`

### No Telemetry

kuk collects zero telemetry. No network calls are made unless you explicitly run `kuk serve`.

---

## Project Architecture

The project is a Cargo workspace with two crates:

```
.
├── Cargo.toml           # Workspace root + kuk package
├── src/                 # kuk — Kanban CLI/TUI/Server
│   ├── main.rs          # Binary entry point
│   ├── lib.rs           # Crate root — re-exports all modules
│   ├── error.rs         # KukError enum with thiserror
│   ├── model/
│   │   ├── card.rs      # Card struct + ULID generation
│   │   ├── board.rs     # Board + Column + card resolution
│   │   ├── config.rs    # RepoConfig
│   │   └── index.rs     # GlobalIndex + IndexEntry
│   ├── storage/
│   │   └── store.rs     # All file I/O (init, load, save)
│   ├── cli/
│   │   ├── mod.rs       # Command dispatch
│   │   └── commands.rs  # Clap definitions + all command handlers
│   ├── tui/
│   │   ├── app.rs       # TUI state machine + keybinding handlers
│   │   └── ui.rs        # ratatui rendering (columns, cards, help)
│   └── server/
│       ├── api.rs       # Axum REST handlers + test suite
│       └── mcp.rs       # MCP JSON-RPC handler (5 tools)
├── tests/
│   └── cli_tests.rs     # kuk integration tests
└── kuk-pm/              # kuk-pm — Project Manager
    ├── Cargo.toml
    ├── src/
    │   ├── main.rs      # Binary entry point
    │   ├── lib.rs       # Crate root
    │   ├── error.rs     # PmError types
    │   ├── cli/         # CLI commands (16 subcommands)
    │   ├── model/       # Sprint, PmProject, GitMetadata, PmConfig
    │   ├── git/         # gitoxide (gix) integration
    │   ├── sync/        # GitHub/GitLab sync (Phase 1)
    │   └── reports/     # Velocity/burndown/roadmap (Phase 2)
    └── tests/
        └── cli_tests.rs # kuk-pm integration tests
```

### Dependencies

**kuk:**

| Crate | Purpose |
|-------|---------|
| `clap` | CLI argument parsing with derive macros |
| `serde` + `serde_json` | JSON serialization/deserialization |
| `ulid` | Time-sortable unique IDs for cards |
| `chrono` | Timestamps with timezone support |
| `thiserror` | Ergonomic error types |
| `dirs` | Cross-platform home directory resolution |
| `colored` | Terminal color output |
| `ratatui` | Terminal UI framework |
| `crossterm` | Cross-platform terminal input/output |
| `axum` | HTTP server framework |
| `tokio` | Async runtime |
| `tower-http` | CORS middleware |

**kuk-pm (additional):**

| Crate | Purpose |
|-------|---------|
| `gix` | Pure-Rust git implementation (gitoxide) |
| `kuk` | Shared types — Card, Board, Store, Config |

### Design Principles

1. **Local-first** — Everything works offline. No network required.
2. **JSON everywhere** — Cards, boards, config, index. All pretty-printed, all git-diff friendly.
3. **Single binary** — `cargo build --release` produces one file that does everything.
4. **Keyboard-first** — Vim bindings in TUI, scriptable CLI with `--json`.
5. **Zero config** — `kuk init` and you're done. Sane defaults for everything.
6. **Actionable errors** — Every error message tells you what to do next.

---

## Testing

kuk is built with aggressive TDD. Every feature has tests written before implementation.

### Test Breakdown

| Module | Tests | Type |
|--------|-------|------|
| **kuk** | | |
| `model::card` | 5 | Unit — schema roundtrips, defaults, uniqueness |
| `model::board` | 9 | Unit — columns, ordering, card resolution |
| `model::config` | 3 | Unit — defaults, roundtrips, missing fields |
| `model::index` | 6 | Unit — add/remove/dedup, roundtrips |
| `storage::store` | 13 | Unit — init, load, save, config persistence, error paths |
| `tui::app` | 40 | Unit — keybindings, modes, board picker |
| `server::api` | 16 | Async — every REST + MCP endpoint |
| `tests/cli_tests` | 35 | Integration — full binary via `assert_cmd` |
| **kuk-pm** | | |
| `cli::commands` | 5 | Unit — branch name slugification |
| `model` | 14 | Unit — PmConfig, GitMetadata, Sprint, PmProject |
| `git` | 7 | Unit — gitoxide repo, branch, commit, tag operations |
| `reports` | 17 | Unit — velocity, burndown, roadmap, stats, release notes |
| `sync` | 5 | Unit — URL parsing, card metadata read/write |
| `tests/cli_tests` | 60 | Integration — all 16 commands via `assert_cmd` |
| **Total** | **237** | |

### Running Tests

```bash
# All tests (both crates)
cargo test --workspace

# kuk only
cargo test -p kuk

# kuk-pm only
cargo test -p kuk-pm

# Unit tests only
cargo test --lib

# Integration tests only
cargo test --test cli_tests

# Specific module
cargo test tui::app
cargo test server::api
cargo test -p kuk-pm git::tests

# With output
cargo test -- --nocapture
```

### Linting

```bash
cargo clippy --workspace -- -W clippy::all    # Zero warnings
cargo fmt --all -- --check                    # Enforced formatting
```

---

## Building from Source

### Requirements

- Rust 1.75+ (edition 2024)
- No system dependencies

### Build

```bash
git clone https://github.com/leslierussell/kuk.git
cd kuk

# Debug build (both crates)
cargo build --workspace

# Release build (optimized)
cargo build --release --workspace

# Strip binaries (optional, saves ~40%)
strip target/release/kuk target/release/kuk-pm

# Result: kuk ~2.7 MB, kuk-pm ~2.9 MB
```

### Cross-Compilation

```bash
# Linux from macOS
rustup target add x86_64-unknown-linux-gnu
cargo build --release --target x86_64-unknown-linux-gnu

# Windows from macOS/Linux
rustup target add x86_64-pc-windows-gnu
cargo build --release --target x86_64-pc-windows-gnu
```

### Performance Targets

| Metric | Target | Actual |
|--------|--------|--------|
| `kuk` binary (stripped) | < 8 MB | 2.7 MB |
| `kuk-pm` binary (stripped) | < 10 MB | 2.9 MB |
| CLI cold start | < 30 ms | ~1 ms |
| TUI frame rate | 60 fps | 60 fps |
| Server start | < 100 ms | ~10 ms |

---

## kuk-pm: Project Manager

`kuk-pm` is the git-native project manager that sits on top of `kuk`. It bridges your Kanban boards with git — auto-creating branches from cards, generating velocity and burndown reports, syncing bidirectionally with GitHub/GitLab, and producing release notes from commit history.

### Setup

```bash
cd my-project
kuk init         # Must be done first
kuk-pm init      # Creates .kuk/pm.json + .kuk/sprints.json
```

### kuk-pm CLI Reference

All commands support `--json`, `--quiet`, and `--repo <PATH>`.

#### Core Commands

```bash
kuk-pm init                    # Initialize kuk-pm in a kuk repo
kuk-pm doctor                  # Health check (kuk, pm, git, boards)
kuk-pm version                 # Print version
kuk-pm projects                # Cross-repo project listing (with git branch info)
```

#### Git Integration

```bash
kuk-pm branch <card-id>        # Create git branch from card title
kuk-pm link <card-id> <url>    # Link card to GitHub issue or PR
kuk-pm pr <card-id>            # Create PR from current branch (via gh CLI)
kuk-pm release-notes [--since tag]  # Generate release notes from git history
```

**Branch creation** reads the card title, slugifies it, and creates a `feature/` branch via gitoxide:

```bash
$ kuk-pm branch 1
Created branch: feature/implement-oauth-login (from card: Implement OAuth login)
```

**Link** stores issue/PR URLs in card metadata (auto-detects type from URL):

```bash
$ kuk-pm link 1 https://github.com/user/repo/issues/42
Linked card 01KJBD... to issue: https://github.com/user/repo/issues/42

$ kuk-pm link 2 https://github.com/user/repo/pull/17
Linked card 01KJBD... to PR: https://github.com/user/repo/pull/17
```

**Release notes** walks real git history and categorizes by conventional commit prefix:

```bash
$ kuk-pm release-notes
Release Notes (since last-tag)
════════════════════════════════════════

Features
────────
  - feat: dark mode support
  - feat: add OAuth provider

Fixes
─────
  - fix: handle empty search query

Other
─────
  - chore: update dependencies

5 commits total
```

#### Sprint Management

```bash
kuk-pm sprint create <name> --start YYYY-MM-DD --end YYYY-MM-DD
kuk-pm sprint close <name>
kuk-pm sprint list
```

```bash
$ kuk-pm sprint create sprint-1 --start 2026-02-17 --end 2026-03-03
Created sprint: sprint-1 (2026-02-17 → 2026-03-03)

$ kuk-pm sprint list
Sprints
───────
  sprint-1 (2026-02-17 → 2026-03-03) [planned]
  sprint-2 (2026-03-03 → 2026-03-17) [planned]

$ kuk-pm sprint close sprint-1
Closed sprint: sprint-1
```

#### Reports & Analytics

```bash
kuk-pm velocity [--weeks 4]         # Cards completed per week with trend
kuk-pm burndown [--sprint <name>]   # Burndown chart (ideal vs actual)
kuk-pm roadmap [--weeks 12]         # Projected card flow with milestones
kuk-pm stats                        # WIP, throughput, cycle time
```

**Velocity** counts done cards per week from real board data:

```bash
$ kuk-pm velocity
Velocity (last 4 weeks)
────────────────────────────────
  2026-02-02    0
  2026-02-09    0
  2026-02-16    0
  2026-02-23    1  ████████████████████

Average: 0.2 cards/week
Trend: → stable
```

**Stats** shows WIP counts, throughput, cycle time, and WIP limit violations:

```bash
$ kuk-pm stats
Project Statistics
──────────────────
Board: default (5 active, 0 archived)

Work in Progress:   2 cards
WIP Limit:          none set
Throughput (7d):    1 cards
Throughput (30d):   1 cards
Avg Cycle Time:     0.0 days
Oldest WIP:         "Implement OAuth login" (0 days)
```

**Burndown** compares ideal vs actual progress for a sprint:

```bash
$ kuk-pm burndown --sprint sprint-1
Burndown: sprint-1 (2026-02-17 → 2026-03-03)
──────────────────────────────────────────────
Total scope: 5 cards

Date         Ideal  Actual  Remaining
2026-02-17    5.0       5  █████
2026-02-24    2.5       5  █████

Status: Behind schedule (5 remaining)
```

**Roadmap** projects card flow using calculated velocity:

```bash
$ kuk-pm roadmap --weeks 6
Roadmap (next 6 weeks, velocity: 0.2/wk)
──────────────────────────────────────────────────
Week          Todo  Doing  Done  Milestones
2026-02-23     2      2     1
2026-03-02     2      2     1  sprint-1 ends
2026-03-09     2      2     1
2026-03-16     2      2     2  sprint-2 ends
2026-03-23     1      2     2
2026-03-30     1      2     2

Estimated completion: ~16 weeks (4 cards remaining)
```

#### Sync

```bash
kuk-pm sync [--dry-run]        # Bidirectional sync with GitHub/GitLab
```

Sync reads linked issue/PR URLs from card metadata and fetches their current state via the `gh` CLI. Closed issues and merged PRs move cards to the "done" column.

```bash
$ kuk-pm sync --dry-run
Dry run — no changes applied:

  [SYNC] Fix login bug — doing → done (issue closed)
  [SKIP] Add dark mode — failed to fetch PR: gh api error: Not Found

2 action(s) (dry run)
```

Requires [GitHub CLI](https://cli.github.com/) (`gh`) to be installed and authenticated.

#### Doctor

```bash
$ kuk-pm doctor
kuk-pm doctor
─────────────
  [OK] .kuk/ directory found
  [OK] kuk config (v0.1.0, board: default)
  [OK] pm.json
  [OK] sprints.json (2 sprints)
  [OK] git repository detected
       └─ branch: main
  [OK] 1 board(s): default
       └─ default: 5 active, 0 archived
  [OK] global index: 1 projects

All checks passed.
```

### kuk-pm Data Model

**`.kuk/pm.json`** — project manager configuration:
```json
{
  "version": "0.1.0",
  "auto_branch": false,
  "sync_provider": null
}
```

**`.kuk/sprints.json`** — sprint definitions:
```json
[
  {
    "name": "Q1-2026",
    "start": "2026-01-01",
    "end": "2026-03-31",
    "goal": "Ship MVP",
    "boards": ["default", "sprint-1"],
    "status": "active"
  }
]
```

**GitMetadata** — per-card git info, stored in `card.metadata["pm"]`:
```json
{
  "branch": "feature/implement-login",
  "issue_url": "https://github.com/user/repo/issues/42",
  "pr_url": "https://github.com/user/repo/pull/43",
  "commits": ["abc123", "def456"],
  "last_synced": "2026-02-25T12:00:00Z"
}
```

This metadata is written by `kuk-pm link`, `kuk-pm pr`, and `kuk-pm sync`, and read by `kuk-pm sync` for bidirectional state tracking.

### Git Integration (gitoxide)

kuk-pm uses [gitoxide](https://github.com/Byron/gitoxide) (`gix`) for all git operations — pure Rust, no shelling out to `git`:

- **Repository detection** — discover `.git/` in any parent directory
- **Branch management** — read current branch, create feature branches
- **Commit history** — traverse ancestors from HEAD, extract message/author/time
- **Tag listing** — enumerate all tags for release note generation
- **Commit ranges** — walk commits between HEAD and a named ref (tag or branch)

For GitHub/GitLab API interaction (`sync`, `pr`), kuk-pm delegates to the [GitHub CLI](https://cli.github.com/) (`gh`) rather than embedding an HTTP client — keeping the binary lean and leveraging the user's existing authentication.

---

## License

MIT

## Assessment
The kuk kanban MCP — is, immediately useful. Having an AI agent that can add cards, move them, and check the board
  during a coding session creates a tight loop. Instead of you mentally tracking "what's left," the agent can check the board, pick up work, and update status as it goes. The fact that it's local JSON in the repo means zero friction — no API tokens, no SaaS, no network dependency.

  The kuk-pm MCP — useful, but it grows with time. Sprint management and linking cards to GitHub issues/PRs are handy from day one. But velocity, burndown, and roadmap only become meaningful after a few weeks of real usage when there's enough data to show trends. Early on those reports will look sparse.

  Where the real value lives:

  - An AI agent can pm_stats to understand project health before suggesting what to work on
  - pm_link + pm_sync close the loop between the board and GitHub without you manually updating either
  - pm_release_notes from real git history is genuinely useful at ship time
  - Branch creation from cards enforces a clean workflow (card → branch → PR → done)

  The honest limitation: AI agents don't spontaneously check the board. You have to prompt it — "what's on the board?" or "move that to done." It's not a passive integration, it's a tool the agent uses when asked. That said, if you build habits around it (or add hooks), it becomes a natural part of the workflow.

  Who this is for: Solo devs and small teams who want project management that lives in the repo, works offline, and
  integrates with AI tooling. If you're already on Linear or Jira with a team of 10, this isn't replacing that. But for the "I just want a board per repo without signing up for anything" crowd — this fills a real gap, and the MCP layer makes it uniquely AI-native.
