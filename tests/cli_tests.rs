use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;

fn kuk() -> Command {
    Command::cargo_bin("kuk").unwrap()
}

fn kuk_in(dir: &TempDir) -> Command {
    let mut cmd = kuk();
    cmd.arg("--repo").arg(dir.path());
    cmd
}

// --- Version ---

#[test]
fn version_prints_version() {
    kuk()
        .arg("version")
        .assert()
        .success()
        .stdout(predicate::str::contains("kuk 0.1.0"));
}

// --- No args ---

#[test]
fn no_args_shows_intro() {
    kuk()
        .assert()
        .success()
        .stdout(predicate::str::contains("Kanban that ships with your code"));
}

// --- Help ---

#[test]
fn help_works() {
    kuk()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("Kanban"));
}

// --- Init ---

#[test]
fn init_creates_kuk_dir() {
    let dir = TempDir::new().unwrap();
    kuk_in(&dir)
        .arg("init")
        .assert()
        .success()
        .stdout(predicate::str::contains("Initialized"));

    assert!(dir.path().join(".kuk").exists());
    assert!(dir.path().join(".kuk/config.json").exists());
    assert!(dir.path().join(".kuk/boards/default.json").exists());
}

#[test]
fn init_twice_fails() {
    let dir = TempDir::new().unwrap();
    kuk_in(&dir).arg("init").assert().success();
    kuk_in(&dir)
        .arg("init")
        .assert()
        .failure()
        .stderr(predicate::str::contains("Already initialized"));
}

// --- Doctor ---

#[test]
fn doctor_before_init() {
    let dir = TempDir::new().unwrap();
    kuk_in(&dir)
        .arg("doctor")
        .assert()
        .success()
        .stdout(predicate::str::contains("[!!] .kuk/ not found"));
}

#[test]
fn doctor_after_init() {
    let dir = TempDir::new().unwrap();
    kuk_in(&dir).arg("init").assert().success();
    kuk_in(&dir)
        .arg("doctor")
        .assert()
        .success()
        .stdout(predicate::str::contains("[OK]"));
}

// --- Add ---

#[test]
fn add_card() {
    let dir = TempDir::new().unwrap();
    kuk_in(&dir).arg("init").assert().success();
    kuk_in(&dir)
        .args(["add", "Build the thing"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Added: Build the thing → todo"));
}

#[test]
fn add_card_to_specific_column() {
    let dir = TempDir::new().unwrap();
    kuk_in(&dir).arg("init").assert().success();
    kuk_in(&dir)
        .args(["add", "In progress task", "--to", "doing"])
        .assert()
        .success()
        .stdout(predicate::str::contains("→ doing"));
}

#[test]
fn add_card_to_invalid_column_fails() {
    let dir = TempDir::new().unwrap();
    kuk_in(&dir).arg("init").assert().success();
    kuk_in(&dir)
        .args(["add", "Bad column", "--to", "nonexistent"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Column not found"));
}

#[test]
fn add_card_with_labels_and_assignee() {
    let dir = TempDir::new().unwrap();
    kuk_in(&dir).arg("init").assert().success();
    kuk_in(&dir)
        .args([
            "add",
            "Labeled task",
            "--label",
            "bug",
            "--label",
            "urgent",
            "--assignee",
            "leslie",
        ])
        .assert()
        .success();
}

#[test]
fn add_card_json_output() {
    let dir = TempDir::new().unwrap();
    kuk_in(&dir).arg("init").assert().success();
    let output = kuk_in(&dir)
        .args(["add", "JSON card", "--json"])
        .output()
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["title"], "JSON card");
    assert_eq!(json["column"], "todo");
}

// --- List ---

#[test]
fn list_empty_board() {
    let dir = TempDir::new().unwrap();
    kuk_in(&dir).arg("init").assert().success();
    kuk_in(&dir)
        .arg("list")
        .assert()
        .success()
        .stdout(predicate::str::contains("TODO (0)"))
        .stdout(predicate::str::contains("DOING (0)"))
        .stdout(predicate::str::contains("DONE (0)"));
}

#[test]
fn list_shows_added_cards() {
    let dir = TempDir::new().unwrap();
    kuk_in(&dir).arg("init").assert().success();
    kuk_in(&dir).args(["add", "First task"]).assert().success();
    kuk_in(&dir).args(["add", "Second task"]).assert().success();
    kuk_in(&dir)
        .arg("list")
        .assert()
        .success()
        .stdout(predicate::str::contains("TODO (2)"))
        .stdout(predicate::str::contains("1. First task"))
        .stdout(predicate::str::contains("2. Second task"));
}

#[test]
fn list_json_output() {
    let dir = TempDir::new().unwrap();
    kuk_in(&dir).arg("init").assert().success();
    kuk_in(&dir).args(["add", "A card"]).assert().success();
    let output = kuk_in(&dir).args(["list", "--json"]).output().unwrap();
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(json["cards"].is_array());
    assert_eq!(json["cards"].as_array().unwrap().len(), 1);
}

// --- Move ---

#[test]
fn move_card_by_number() {
    let dir = TempDir::new().unwrap();
    kuk_in(&dir).arg("init").assert().success();
    kuk_in(&dir).args(["add", "Move me"]).assert().success();
    kuk_in(&dir)
        .args(["move", "1", "--to", "doing"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Moved: Move me → doing"));
}

#[test]
fn move_nonexistent_card_fails() {
    let dir = TempDir::new().unwrap();
    kuk_in(&dir).arg("init").assert().success();
    kuk_in(&dir)
        .args(["move", "99", "--to", "doing"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Card not found"));
}

#[test]
fn move_to_invalid_column_fails() {
    let dir = TempDir::new().unwrap();
    kuk_in(&dir).arg("init").assert().success();
    kuk_in(&dir).args(["add", "Card"]).assert().success();
    kuk_in(&dir)
        .args(["move", "1", "--to", "nope"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Column not found"));
}

// --- Archive ---

#[test]
fn archive_card() {
    let dir = TempDir::new().unwrap();
    kuk_in(&dir).arg("init").assert().success();
    kuk_in(&dir).args(["add", "Archive me"]).assert().success();
    kuk_in(&dir)
        .args(["archive", "1"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Archived"));

    // Should not show in list
    kuk_in(&dir)
        .arg("list")
        .assert()
        .success()
        .stdout(predicate::str::contains("TODO (0)"));
}

// --- Delete ---

#[test]
fn delete_card() {
    let dir = TempDir::new().unwrap();
    kuk_in(&dir).arg("init").assert().success();
    kuk_in(&dir).args(["add", "Delete me"]).assert().success();
    kuk_in(&dir)
        .args(["delete", "1"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Deleted"));

    kuk_in(&dir)
        .arg("list")
        .assert()
        .success()
        .stdout(predicate::str::contains("TODO (0)"));
}

// --- Hoist / Demote ---

#[test]
fn hoist_card() {
    let dir = TempDir::new().unwrap();
    kuk_in(&dir).arg("init").assert().success();
    kuk_in(&dir).args(["add", "First"]).assert().success();
    kuk_in(&dir).args(["add", "Second"]).assert().success();
    kuk_in(&dir)
        .args(["hoist", "2"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Hoisted"));
}

#[test]
fn demote_card() {
    let dir = TempDir::new().unwrap();
    kuk_in(&dir).arg("init").assert().success();
    kuk_in(&dir).args(["add", "First"]).assert().success();
    kuk_in(&dir).args(["add", "Second"]).assert().success();
    kuk_in(&dir)
        .args(["demote", "1"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Demoted"));
}

// --- Label ---

#[test]
fn label_add_and_remove() {
    let dir = TempDir::new().unwrap();
    kuk_in(&dir).arg("init").assert().success();
    kuk_in(&dir).args(["add", "Labelable"]).assert().success();
    kuk_in(&dir)
        .args(["label", "1", "add", "bug"])
        .assert()
        .success()
        .stdout(predicate::str::contains("bug"));

    kuk_in(&dir)
        .args(["label", "1", "remove", "bug"])
        .assert()
        .success();
}

#[test]
fn label_remove_nonexistent_fails() {
    let dir = TempDir::new().unwrap();
    kuk_in(&dir).arg("init").assert().success();
    kuk_in(&dir).args(["add", "No label"]).assert().success();
    kuk_in(&dir)
        .args(["label", "1", "remove", "nope"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Label not found"));
}

// --- Assign ---

#[test]
fn assign_user() {
    let dir = TempDir::new().unwrap();
    kuk_in(&dir).arg("init").assert().success();
    kuk_in(&dir).args(["add", "Assignable"]).assert().success();
    kuk_in(&dir)
        .args(["assign", "1", "leslie"])
        .assert()
        .success()
        .stdout(predicate::str::contains("@leslie"));
}

// --- Board commands ---

#[test]
fn board_list_shows_active_marker() {
    let dir = TempDir::new().unwrap();
    kuk_in(&dir).arg("init").assert().success();
    kuk_in(&dir)
        .args(["board", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("* default"));
}

#[test]
fn board_create() {
    let dir = TempDir::new().unwrap();
    kuk_in(&dir).arg("init").assert().success();
    kuk_in(&dir)
        .args(["board", "create", "sprint-1"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Created board: sprint-1"));

    kuk_in(&dir)
        .args(["board", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("sprint-1"));
}

#[test]
fn board_create_duplicate_fails() {
    let dir = TempDir::new().unwrap();
    kuk_in(&dir).arg("init").assert().success();
    kuk_in(&dir)
        .args(["board", "create", "default"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("already exists"));
}

#[test]
fn board_switch_persists() {
    let dir = TempDir::new().unwrap();
    kuk_in(&dir).arg("init").assert().success();
    kuk_in(&dir)
        .args(["board", "create", "sprint-1"])
        .assert()
        .success();

    // Switch to sprint-1
    kuk_in(&dir)
        .args(["board", "switch", "sprint-1"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Switched to board: sprint-1"));

    // board list should show * on sprint-1, not default
    kuk_in(&dir)
        .args(["board", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("* sprint-1"))
        .stdout(predicate::str::contains("  default"));

    // add should go to the active board (sprint-1)
    kuk_in(&dir)
        .args(["add", "Task on sprint"])
        .assert()
        .success();

    // list should show it on sprint-1 (the active board)
    kuk_in(&dir)
        .arg("list")
        .assert()
        .success()
        .stdout(predicate::str::contains("Task on sprint"));

    // default board should still be empty
    kuk_in(&dir)
        .args(["list", "--board", "default"])
        .assert()
        .success()
        .stdout(predicate::str::contains("TODO (0)"));
}

#[test]
fn board_switch_nonexistent_fails() {
    let dir = TempDir::new().unwrap();
    kuk_in(&dir).arg("init").assert().success();
    kuk_in(&dir)
        .args(["board", "switch", "nope"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Board not found"));
}

#[test]
fn board_switch_survives_other_commands() {
    let dir = TempDir::new().unwrap();
    kuk_in(&dir).arg("init").assert().success();
    kuk_in(&dir)
        .args(["board", "create", "backlog"])
        .assert()
        .success();
    kuk_in(&dir)
        .args(["board", "switch", "backlog"])
        .assert()
        .success();

    // Run several commands, then verify we're still on backlog
    kuk_in(&dir).arg("version").assert().success();
    kuk_in(&dir).arg("doctor").assert().success();
    kuk_in(&dir)
        .args(["board", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("* backlog"));
}

// --- Projects ---

#[test]
fn projects_command_runs() {
    // Just test it doesn't crash; global index depends on home dir
    kuk().arg("projects").assert().success();
}

// --- Commands before init ---

#[test]
fn list_before_init_fails() {
    let dir = TempDir::new().unwrap();
    kuk_in(&dir)
        .arg("list")
        .assert()
        .failure()
        .stderr(predicate::str::contains("Run `kuk init` first"));
}

#[test]
fn add_before_init_fails() {
    let dir = TempDir::new().unwrap();
    kuk_in(&dir)
        .args(["add", "oops"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Run `kuk init` first"));
}

#[test]
fn move_before_init_fails() {
    let dir = TempDir::new().unwrap();
    kuk_in(&dir)
        .args(["move", "1", "--to", "doing"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Run `kuk init` first"));
}
