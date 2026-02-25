**PROJECT: kuk**  
**Version: 0.1.0 MVP → 1.0**  
**Date: 2026-02-24**  
**Owner: Leslie (you)**  
**Language: Rust (non-negotiable for single-binary, speed, TUI quality)**

### 1. Vision (One-liner + Elevator Pitch)
**One-liner:**  
`kuk` = the git-native, JSON-first, AI-native, repo-scoped Kanban that lives in your terminal and your agents.

**Elevator Pitch:**  
A single `kuk` binary that turns every git repo into a fully functional Kanban board stored as plain JSON inside `.kuk/`. Zero config for 95% of solo devs. Full REST + MCP server when you run `kuk serve`. Later a tiny micro-SaaS (kuk.cloud) with metered API keys and org RBAC. It must feel like `git` had a baby with `taskell` + `clikan` + Claude’s tool-use.

**Tagline:** “Kanban that ships with your code.”

### 2. Differentiation (Devil’s Advocate will grill you on this)
Existing tools (as of Feb 2026):
- kanban-tui / kanban-cli (Rust crates)
- basilk, rust_kanban, taskell, clikan, kanban.bash

**kuk wins because:**
- Repo-scoped + global `kuk projects` index (no other tool does this)
- Pure JSON storage → git diff friendly, AI can read/write perfectly
- Built-in MCP server (Model Context Protocol) so any agent can `use_kuk`
- Single binary does CLI + TUI + server + future web
- Local-first forever, cloud optional and metered
- Designed from day 1 for aggressive TDD + 95%+ test coverage

### 3. Product Requirements Document (PRD) – Exhaustive

#### 3.1 User Personas
1. **Solo Dev (80% of users)** – Leslie in Florence, AL. Wants `kuk` in 50 repos. Zero cloud.
2. **Indie Hacker / Small Team** – needs API keys + basic org.
3. **AI Power User** – wants MCP so Cursor/Claude can move cards automatically.
4. **Enterprise Dev** – SSO, audit, on-prem.

#### 3.2 Core Principles (non-negotiable)
- Local-first, offline-first, git-friendly
- Everything is JSON (cards, boards, index, config)
- Keyboard-first (vim bindings in TUI)
- Scriptable CLI (every action is a subcommand)
- Zero runtime dependencies on install
- Single static binary (`cargo build --release`)
- 95%+ test coverage enforced
- All errors are actionable and human-readable

#### 3.3 Data Model (v1 – must be frozen after MVP)
```json
~/.kuk/index.json          // global repo registry
.kuk/config.json           // per-repo settings
.kuk/boards/default.json   // or myboard.json
```

Card schema (full):
```json
{
  "id": "ulid or uuidv7",
  "title": string,
  "column": "todo" | "doing" | ...,
  "order": u32,                    // within column
  "description": string | null,
  "assignee": string | null,
  "labels": string[],
  "due": ISO8601 | null,
  "created_at": ISO8601,
  "updated_at": ISO8601,
  "metadata": object,              // pr_url, issue_url, etc.
  "archived": bool
}
```

Board has columns array with optional wipLimit.

#### 3.4 CLI Commands – Exhaustive Spec (MVP)
All commands support `--json`, `--quiet`, `--repo <path>`

```bash
kuk init [--board-name default]
kuk projects [--remote]                    # lists every .kuk repo on system
kuk list [--board default] [--json]
kuk add <title> [--to todo] [--label x] [--assignee me]
kuk move <id|number> --to <column>
kuk hoist <id|number>                      # top of current column
kuk demote <id|number>                     # bottom of current column
kuk edit <id|number>                       # $EDITOR on card JSON
kuk archive <id|number>
kuk delete <id|number>
kuk label <id> add/remove <tag>
kuk assign <id> <user>
kuk board create <name>
kuk board switch <name>
kuk board list
kuk serve [--port 8080] [--mcp]            # starts API + MCP
kuk version
kuk config
kuk doctor                                 # health check
```

All numeric IDs in `list` output are 1-based per column for human speed.

#### 3.5 TUI Requirements (--tui flag or `kuk tui`)
- ratatui + crossterm
- Vim keys: j/k/h/l, gg/G, / search, dd delete, etc.
- Modal editing (normal / insert)
- Live preview of JSON changes
- Must run in < 50ms cold start

#### 3.6 API & MCP (Phase 3)
- Axum / tower
- OpenAPI spec (must ship with binary)
- Endpoints: `/v1/boards`, `/v1/cards`, `/v1/move`, etc.
- MCP endpoint at `/mcp` (standard Model Context Protocol)
- API keys with usage metering (even locally for future cloud sync)

#### 3.7 Cloud / SaaS (Phase 4+)
- kuk.cloud
- Tiers: Solo (free), Pro ($29), Enterprise ($99+)
- Org → RBAC → Projects → API keys (metered by calls + storage)
- Sync daemon (`kuk sync`)

#### 3.8 Non-Functional
- Binary size < 8 MB stripped
- Cold start CLI < 30 ms
- TUI 60 fps
- 100% offline capable
- Git-safe (never writes outside .kuk/)
- Security: no telemetry unless opted-in

### 4. Aggressively TDD-Driven Development Plan

**Golden Rule:**  
**Red → Green → Refactor. No code without a failing test first.**  
Coverage enforced via `cargo tarpaulin --ignore-tests --min-coverage 95`

#### Phase 0 – Foundations (Day 1)
1. Project init (`cargo new kuk --bin`)
2. Write failing test for CLI structure (`assert_cmd`)
3. Define exact JSON schemas + serde tests (roundtrip must be lossless)
4. Global index + per-repo storage layer with 20+ unit tests
5. `kuk doctor` + `kuk version`

#### Phase 1 – Core CLI MVP (Days 2-3)
For **every** command:
- Write integration test first (using tempdir + assert_cmd)
- Write unit tests for domain logic
- Implement minimal code to make tests pass
- Refactor

Specific tests required before any merge:
- `kuk init` idempotent test
- `kuk add` → correct column + order=0
- `kuk move` with non-existent card → clear error
- `kuk projects` discovers nested repos correctly
- All commands with `--json` output valid schema
- 100% error path coverage (invalid ID, missing board, permission, etc.)

#### Phase 2 – TUI (Days 4-5)
- Every keybinding has its own test (using ratatui testing harness or crossterm events)
- Snapshot tests for rendered frames (insta crate)
- TUI must pass same integration tests as CLI

#### Phase 3 – Server + MCP (Day 6-7)
- API tests with `reqwest` + `wiremock`
- MCP protocol compliance tests (official MCP test suite)
- `kuk serve` must start in < 100 ms

#### Phase 4 – Cloud & Polish (Week 2+)
- Stripe integration (mocked first)
- RBAC matrix tests (every role + permission combination)

**CI/CD Requirements (must be in repo from day 1)**
- GitHub Actions: test, tarpaulin, clippy, rustfmt, release binary
- Release workflow builds Linux/macOS/Windows binaries

### 5. Devil’s Advocate / Antagonistic Manager Persona
**Name: Skeptical Steve** (your built-in PM from hell)

**Instructions for Claude:**
After every feature implementation or design decision, you **MUST** switch to Skeptical Steve and answer these questions in a separate section called **SKEPTICAL STEVE SAYS**:

1. Why the hell are we doing it this way instead of the simplest possible thing?
2. How does this add scope creep? Kill it.
3. What’s the test that would have caught us if we shipped without it?
4. Is this still local-first? Prove it.
5. How does this help or hurt the solo dev who never wants cloud?
6. Binary size / startup time impact?
7. Git diff friendliness after this change?
8. If Leslie types `kuk` with no args at 2 a.m., what happens and why is it perfect?

If any answer is weak, **reject the PR** and force redesign.

Steve is allowed to be rude, sarcastic, and obsessive about simplicity.

Example:
> **SKEPTICAL STEVE SAYS:**  
> “Another crate? Really? We’re already at 12 dependencies. Explain why ratatui is worth the binary bloat or I’m nuking it for a 50-line custom TUI.”

### 6. Acceptance Criteria for MVP (shippable in < 7 days)
- `kuk init && kuk add "hello" && kuk list` works perfectly
- `kuk projects` works across machine
- `kuk --tui` usable with vim keys
- `kuk serve` + basic REST + MCP working
- 95%+ coverage
- Binaries for macOS + Linux
- Full README with demo gif (you’ll make it)

---

“Follow this PRD and TDD plan exactly. Start with Phase 0. After every major step, include the SKEPTICAL STEVE review. Begin.”

We will iterate brutally until it’s perfect.
