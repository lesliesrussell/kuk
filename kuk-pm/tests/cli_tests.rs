use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;

fn kuk() -> Command {
    Command::cargo_bin("kuk").unwrap()
}

fn kuk_pm() -> Command {
    Command::cargo_bin("kuk-pm").unwrap()
}

fn kuk_in(dir: &TempDir) -> Command {
    let mut cmd = kuk();
    cmd.arg("--repo").arg(dir.path());
    cmd
}

fn kuk_pm_in(dir: &TempDir) -> Command {
    let mut cmd = kuk_pm();
    cmd.arg("--repo").arg(dir.path());
    cmd
}

fn init_both(dir: &TempDir) {
    kuk_in(dir).arg("init").assert().success();
    kuk_pm_in(dir).arg("init").assert().success();
}

fn init_git_repo(dir: &TempDir) {
    std::process::Command::new("git")
        .args(["init"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["config", "user.name", "Test"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    std::fs::write(dir.path().join("README.md"), "# Test").unwrap();
    std::process::Command::new("git")
        .args(["add", "."])
        .current_dir(dir.path())
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["commit", "-m", "Initial commit"])
        .current_dir(dir.path())
        .output()
        .unwrap();
}

fn init_git_and_kuk(dir: &TempDir) {
    init_git_repo(dir);
    init_both(dir);
}

fn add_git_commits(dir: &TempDir, messages: &[&str]) {
    for msg in messages {
        let filename = format!("{}.txt", msg.replace(' ', "_"));
        std::fs::write(dir.path().join(&filename), msg).unwrap();
        std::process::Command::new("git")
            .args(["add", &filename])
            .current_dir(dir.path())
            .output()
            .unwrap();
        std::process::Command::new("git")
            .args(["commit", "-m", msg])
            .current_dir(dir.path())
            .output()
            .unwrap();
    }
}

// ─── Version ─────────────────────────────────────────────────

#[test]
fn version_prints_version() {
    kuk_pm()
        .arg("version")
        .assert()
        .success()
        .stdout(predicate::str::contains("kuk-pm 0.1.0"));
}

// ─── No args ─────────────────────────────────────────────────

#[test]
fn no_args_shows_intro() {
    kuk_pm()
        .assert()
        .success()
        .stdout(predicate::str::contains("Git-native project manager"));
}

// ─── Help ────────────────────────────────────────────────────

#[test]
fn help_works() {
    kuk_pm()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("project manager"));
}

// ─── Init ────────────────────────────────────────────────────

#[test]
fn init_creates_pm_files() {
    let dir = TempDir::new().unwrap();
    kuk_in(&dir).arg("init").assert().success();
    kuk_pm_in(&dir)
        .arg("init")
        .assert()
        .success()
        .stdout(predicate::str::contains("Initialized kuk-pm"));

    assert!(dir.path().join(".kuk/pm.json").exists());
    assert!(dir.path().join(".kuk/sprints.json").exists());
}

#[test]
fn init_before_kuk_init_fails() {
    let dir = TempDir::new().unwrap();
    kuk_pm_in(&dir)
        .arg("init")
        .assert()
        .failure()
        .stderr(predicate::str::contains("kuk init"));
}

#[test]
fn init_twice_fails() {
    let dir = TempDir::new().unwrap();
    kuk_in(&dir).arg("init").assert().success();
    kuk_pm_in(&dir).arg("init").assert().success();
    kuk_pm_in(&dir)
        .arg("init")
        .assert()
        .failure()
        .stderr(predicate::str::contains("Already initialized"));
}

#[test]
fn init_detects_git_repo() {
    let dir = TempDir::new().unwrap();
    init_git_repo(&dir);
    kuk_in(&dir).arg("init").assert().success();
    kuk_pm_in(&dir)
        .arg("init")
        .assert()
        .success()
        .stdout(predicate::str::contains("git repo detected"));
}

#[test]
fn init_detects_no_git() {
    let dir = TempDir::new().unwrap();
    kuk_in(&dir).arg("init").assert().success();
    kuk_pm_in(&dir)
        .arg("init")
        .assert()
        .success()
        .stdout(predicate::str::contains("no git repo"));
}

// ─── Doctor ──────────────────────────────────────────────────

#[test]
fn doctor_before_kuk_init() {
    let dir = TempDir::new().unwrap();
    kuk_pm_in(&dir)
        .arg("doctor")
        .assert()
        .success()
        .stdout(predicate::str::contains("[!!] .kuk/ not found"));
}

#[test]
fn doctor_after_kuk_init_only() {
    let dir = TempDir::new().unwrap();
    kuk_in(&dir).arg("init").assert().success();
    kuk_pm_in(&dir)
        .arg("doctor")
        .assert()
        .success()
        .stdout(predicate::str::contains("[OK] .kuk/ directory found"))
        .stdout(predicate::str::contains("[--] pm.json not found"));
}

#[test]
fn doctor_after_full_init() {
    let dir = TempDir::new().unwrap();
    init_both(&dir);
    kuk_pm_in(&dir)
        .arg("doctor")
        .assert()
        .success()
        .stdout(predicate::str::contains("[OK] pm.json"))
        .stdout(predicate::str::contains("[OK] sprints.json"));
}

#[test]
fn doctor_with_git() {
    let dir = TempDir::new().unwrap();
    init_git_and_kuk(&dir);
    kuk_pm_in(&dir)
        .arg("doctor")
        .assert()
        .success()
        .stdout(predicate::str::contains("[OK] git repository detected"));
}

// ─── Projects ────────────────────────────────────────────────

#[test]
fn projects_command_runs() {
    kuk_pm().arg("projects").assert().success();
}

// ─── Branch ──────────────────────────────────────────────────

#[test]
fn branch_creates_git_branch() {
    let dir = TempDir::new().unwrap();
    init_git_and_kuk(&dir);

    kuk_in(&dir)
        .args(["add", "Implement login"])
        .assert()
        .success();

    kuk_pm_in(&dir)
        .args(["branch", "1"])
        .assert()
        .success()
        .stdout(predicate::str::contains("feature/implement-login"));
}

#[test]
fn branch_nonexistent_card_fails() {
    let dir = TempDir::new().unwrap();
    init_git_and_kuk(&dir);
    kuk_pm_in(&dir)
        .args(["branch", "99"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Card not found"));
}

#[test]
fn branch_without_git_fails() {
    let dir = TempDir::new().unwrap();
    init_both(&dir);
    kuk_in(&dir).args(["add", "No git"]).assert().success();
    kuk_pm_in(&dir)
        .args(["branch", "1"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Not a git repository"));
}

#[test]
fn branch_json_output() {
    let dir = TempDir::new().unwrap();
    init_git_and_kuk(&dir);
    kuk_in(&dir)
        .args(["add", "JSON branch test"])
        .assert()
        .success();

    let output = kuk_pm_in(&dir)
        .args(["branch", "1", "--json"])
        .output()
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["branch"], "feature/json-branch-test");
    assert_eq!(json["title"], "JSON branch test");
}

// ─── Sprint CRUD ─────────────────────────────────────────────

#[test]
fn sprint_create_and_list() {
    let dir = TempDir::new().unwrap();
    init_both(&dir);

    kuk_pm_in(&dir)
        .args([
            "sprint",
            "create",
            "sprint-1",
            "--start",
            "2026-03-01",
            "--end",
            "2026-03-14",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Created sprint: sprint-1"));

    kuk_pm_in(&dir)
        .args(["sprint", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("sprint-1"))
        .stdout(predicate::str::contains("2026-03-01"))
        .stdout(predicate::str::contains("planned"));
}

#[test]
fn sprint_create_json() {
    let dir = TempDir::new().unwrap();
    init_both(&dir);

    let output = kuk_pm_in(&dir)
        .args([
            "sprint",
            "create",
            "s1",
            "--start",
            "2026-03-01",
            "--end",
            "2026-03-14",
            "--json",
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["name"], "s1");
    assert_eq!(json["status"], "planned");
}

#[test]
fn sprint_create_duplicate_fails() {
    let dir = TempDir::new().unwrap();
    init_both(&dir);

    kuk_pm_in(&dir)
        .args([
            "sprint",
            "create",
            "dup",
            "--start",
            "2026-03-01",
            "--end",
            "2026-03-14",
        ])
        .assert()
        .success();

    kuk_pm_in(&dir)
        .args([
            "sprint",
            "create",
            "dup",
            "--start",
            "2026-04-01",
            "--end",
            "2026-04-14",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("already exists"));
}

#[test]
fn sprint_create_invalid_dates() {
    let dir = TempDir::new().unwrap();
    init_both(&dir);

    // End before start
    kuk_pm_in(&dir)
        .args([
            "sprint",
            "create",
            "bad",
            "--start",
            "2026-03-14",
            "--end",
            "2026-03-01",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("end date must be after start"));
}

#[test]
fn sprint_create_bad_date_format() {
    let dir = TempDir::new().unwrap();
    init_both(&dir);

    kuk_pm_in(&dir)
        .args([
            "sprint",
            "create",
            "bad",
            "--start",
            "not-a-date",
            "--end",
            "2026-03-14",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("invalid"));
}

#[test]
fn sprint_close() {
    let dir = TempDir::new().unwrap();
    init_both(&dir);

    kuk_pm_in(&dir)
        .args([
            "sprint",
            "create",
            "s1",
            "--start",
            "2026-03-01",
            "--end",
            "2026-03-14",
        ])
        .assert()
        .success();

    kuk_pm_in(&dir)
        .args(["sprint", "close", "s1"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Closed sprint: s1"));

    // Verify it shows as closed
    kuk_pm_in(&dir)
        .args(["sprint", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("closed"));
}

#[test]
fn sprint_close_nonexistent_fails() {
    let dir = TempDir::new().unwrap();
    init_both(&dir);

    kuk_pm_in(&dir)
        .args(["sprint", "close", "no-such-sprint"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Sprint not found"));
}

#[test]
fn sprint_close_already_closed_fails() {
    let dir = TempDir::new().unwrap();
    init_both(&dir);

    kuk_pm_in(&dir)
        .args([
            "sprint",
            "create",
            "s1",
            "--start",
            "2026-03-01",
            "--end",
            "2026-03-14",
        ])
        .assert()
        .success();

    kuk_pm_in(&dir)
        .args(["sprint", "close", "s1"])
        .assert()
        .success();

    kuk_pm_in(&dir)
        .args(["sprint", "close", "s1"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("already closed"));
}

#[test]
fn sprint_list_empty() {
    let dir = TempDir::new().unwrap();
    init_both(&dir);

    kuk_pm_in(&dir)
        .args(["sprint", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("No sprints defined"));
}

#[test]
fn sprint_list_json() {
    let dir = TempDir::new().unwrap();
    init_both(&dir);

    kuk_pm_in(&dir)
        .args([
            "sprint",
            "create",
            "s1",
            "--start",
            "2026-03-01",
            "--end",
            "2026-03-14",
        ])
        .assert()
        .success();

    let output = kuk_pm_in(&dir)
        .args(["sprint", "list", "--json"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(json.is_array());
    assert_eq!(json.as_array().unwrap().len(), 1);
    assert_eq!(json[0]["name"], "s1");
}

// ─── Link ────────────────────────────────────────────────────

#[test]
fn link_issue_to_card() {
    let dir = TempDir::new().unwrap();
    init_both(&dir);
    kuk_in(&dir)
        .args(["add", "Login feature"])
        .assert()
        .success();

    kuk_pm_in(&dir)
        .args(["link", "1", "https://github.com/user/repo/issues/42"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Linked card"))
        .stdout(predicate::str::contains("issue"));
}

#[test]
fn link_pr_to_card() {
    let dir = TempDir::new().unwrap();
    init_both(&dir);
    kuk_in(&dir)
        .args(["add", "Login feature"])
        .assert()
        .success();

    kuk_pm_in(&dir)
        .args(["link", "1", "https://github.com/user/repo/pull/7"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Linked card"))
        .stdout(predicate::str::contains("PR"));
}

#[test]
fn link_json_output() {
    let dir = TempDir::new().unwrap();
    init_both(&dir);
    kuk_in(&dir).args(["add", "Test card"]).assert().success();

    let output = kuk_pm_in(&dir)
        .args(["link", "1", "https://github.com/u/r/issues/1", "--json"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["type"], "issue");
    assert!(json["url"].as_str().unwrap().contains("issues"));
}

#[test]
fn link_nonexistent_card_fails() {
    let dir = TempDir::new().unwrap();
    init_both(&dir);

    kuk_pm_in(&dir)
        .args(["link", "99", "https://github.com/u/r/issues/1"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Card not found"));
}

#[test]
fn link_before_init_fails() {
    let dir = TempDir::new().unwrap();
    kuk_pm_in(&dir)
        .args(["link", "1", "https://github.com/u/r/issues/1"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("kuk init"));
}

// ─── Velocity ────────────────────────────────────────────────

#[test]
fn velocity_shows_report() {
    let dir = TempDir::new().unwrap();
    init_both(&dir);

    kuk_pm_in(&dir)
        .arg("velocity")
        .assert()
        .success()
        .stdout(predicate::str::contains("Velocity"))
        .stdout(predicate::str::contains("Average"))
        .stdout(predicate::str::contains("Trend"));
}

#[test]
fn velocity_json() {
    let dir = TempDir::new().unwrap();
    init_both(&dir);

    let output = kuk_pm_in(&dir)
        .args(["velocity", "--json"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(json["weeks"].is_array());
    assert!(json["average"].is_number());
    assert!(json["trend"].is_string());
}

#[test]
fn velocity_custom_weeks() {
    let dir = TempDir::new().unwrap();
    init_both(&dir);

    kuk_pm_in(&dir)
        .args(["velocity", "--weeks", "8"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Velocity (last 8 weeks)"));
}

#[test]
fn velocity_before_init_fails() {
    let dir = TempDir::new().unwrap();
    kuk_pm_in(&dir)
        .arg("velocity")
        .assert()
        .failure()
        .stderr(predicate::str::contains("kuk init"));
}

// ─── Stats ───────────────────────────────────────────────────

#[test]
fn stats_shows_report() {
    let dir = TempDir::new().unwrap();
    init_both(&dir);

    kuk_pm_in(&dir)
        .arg("stats")
        .assert()
        .success()
        .stdout(predicate::str::contains("Project Statistics"))
        .stdout(predicate::str::contains("Work in Progress"))
        .stdout(predicate::str::contains("Throughput"));
}

#[test]
fn stats_json() {
    let dir = TempDir::new().unwrap();
    init_both(&dir);

    let output = kuk_pm_in(&dir).args(["stats", "--json"]).output().unwrap();
    assert!(output.status.success());
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["board_name"], "default");
    assert!(json["wip_count"].is_number());
}

#[test]
fn stats_with_cards() {
    let dir = TempDir::new().unwrap();
    init_both(&dir);
    kuk_in(&dir).args(["add", "Task A"]).assert().success();
    kuk_in(&dir).args(["add", "Task B"]).assert().success();

    kuk_pm_in(&dir)
        .arg("stats")
        .assert()
        .success()
        .stdout(predicate::str::contains("2 active"));
}

#[test]
fn stats_before_init_fails() {
    let dir = TempDir::new().unwrap();
    kuk_pm_in(&dir)
        .arg("stats")
        .assert()
        .failure()
        .stderr(predicate::str::contains("kuk init"));
}

// ─── Burndown ────────────────────────────────────────────────

#[test]
fn burndown_with_sprint() {
    let dir = TempDir::new().unwrap();
    init_both(&dir);

    // Create a sprint
    kuk_pm_in(&dir)
        .args([
            "sprint",
            "create",
            "s1",
            "--start",
            "2026-02-01",
            "--end",
            "2026-03-01",
        ])
        .assert()
        .success();

    kuk_pm_in(&dir)
        .args(["burndown", "--sprint", "s1"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Burndown: s1"));
}

#[test]
fn burndown_no_sprint_fails() {
    let dir = TempDir::new().unwrap();
    init_both(&dir);

    // No active sprint
    kuk_pm_in(&dir)
        .arg("burndown")
        .assert()
        .failure()
        .stderr(predicate::str::contains("No active sprint"));
}

#[test]
fn burndown_nonexistent_sprint_fails() {
    let dir = TempDir::new().unwrap();
    init_both(&dir);

    kuk_pm_in(&dir)
        .args(["burndown", "--sprint", "nope"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Sprint not found"));
}

#[test]
fn burndown_json() {
    let dir = TempDir::new().unwrap();
    init_both(&dir);

    kuk_pm_in(&dir)
        .args([
            "sprint",
            "create",
            "s1",
            "--start",
            "2026-02-01",
            "--end",
            "2026-03-01",
        ])
        .assert()
        .success();

    let output = kuk_pm_in(&dir)
        .args(["burndown", "--sprint", "s1", "--json"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["sprint_name"], "s1");
    assert!(json["total_cards"].is_number());
}

#[test]
fn burndown_before_init_fails() {
    let dir = TempDir::new().unwrap();
    kuk_pm_in(&dir)
        .arg("burndown")
        .assert()
        .failure()
        .stderr(predicate::str::contains("kuk init"));
}

// ─── Roadmap ─────────────────────────────────────────────────

#[test]
fn roadmap_shows_report() {
    let dir = TempDir::new().unwrap();
    init_both(&dir);

    kuk_pm_in(&dir)
        .arg("roadmap")
        .assert()
        .success()
        .stdout(predicate::str::contains("Roadmap"))
        .stdout(predicate::str::contains("Todo"))
        .stdout(predicate::str::contains("Done"));
}

#[test]
fn roadmap_custom_weeks() {
    let dir = TempDir::new().unwrap();
    init_both(&dir);

    kuk_pm_in(&dir)
        .args(["roadmap", "--weeks", "6"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Roadmap (next 6 weeks"));
}

#[test]
fn roadmap_json() {
    let dir = TempDir::new().unwrap();
    init_both(&dir);

    let output = kuk_pm_in(&dir)
        .args(["roadmap", "--json"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(json["weeks"].is_array());
    assert!(json["velocity"].is_number());
}

#[test]
fn roadmap_before_init_fails() {
    let dir = TempDir::new().unwrap();
    kuk_pm_in(&dir)
        .arg("roadmap")
        .assert()
        .failure()
        .stderr(predicate::str::contains("kuk init"));
}

// ─── Release Notes ───────────────────────────────────────────

#[test]
fn release_notes_with_git() {
    let dir = TempDir::new().unwrap();
    init_git_and_kuk(&dir);

    add_git_commits(
        &dir,
        &["feat: add login", "fix: null pointer", "chore: update deps"],
    );

    kuk_pm_in(&dir)
        .arg("release-notes")
        .assert()
        .success()
        .stdout(predicate::str::contains("Release Notes"))
        .stdout(predicate::str::contains("commits total"));
}

#[test]
fn release_notes_json() {
    let dir = TempDir::new().unwrap();
    init_git_and_kuk(&dir);

    add_git_commits(&dir, &["feat: add feature", "fix: bug"]);

    let output = kuk_pm_in(&dir)
        .args(["release-notes", "--json"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(json["features"].is_array());
    assert!(json["fixes"].is_array());
}

#[test]
fn release_notes_categorizes_commits() {
    let dir = TempDir::new().unwrap();
    init_git_and_kuk(&dir);

    add_git_commits(
        &dir,
        &[
            "feat: new search",
            "feat: dark mode",
            "fix: crash on empty input",
        ],
    );

    let output = kuk_pm_in(&dir)
        .args(["release-notes", "--json"])
        .output()
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["features"].as_array().unwrap().len(), 2);
    assert_eq!(json["fixes"].as_array().unwrap().len(), 1);
}

#[test]
fn release_notes_without_git_fails() {
    let dir = TempDir::new().unwrap();
    kuk_pm_in(&dir)
        .arg("release-notes")
        .assert()
        .failure()
        .stderr(predicate::str::contains("Not a git repository"));
}

// ─── Sync ────────────────────────────────────────────────────

#[test]
fn sync_requires_init() {
    let dir = TempDir::new().unwrap();
    kuk_pm_in(&dir)
        .arg("sync")
        .assert()
        .failure()
        .stderr(predicate::str::contains("kuk init"));
}

#[test]
fn sync_dry_run_no_linked_cards() {
    let dir = TempDir::new().unwrap();
    init_both(&dir);

    // sync with no linked cards should succeed and show "up to date"
    // (only if gh CLI is available, otherwise we get a graceful error)
    let output = kuk_pm_in(&dir)
        .args(["sync", "--dry-run"])
        .output()
        .unwrap();

    // Either succeeds (gh available) or fails with gh error (acceptable)
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stdout.contains("up to date") || stderr.contains("gh"),
        "Expected 'up to date' or gh error, got stdout={stdout} stderr={stderr}"
    );
}

// ─── PR ──────────────────────────────────────────────────────

#[test]
fn pr_requires_init() {
    let dir = TempDir::new().unwrap();
    kuk_pm_in(&dir)
        .args(["pr", "1"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("kuk init"));
}

#[test]
fn pr_requires_git() {
    let dir = TempDir::new().unwrap();
    init_both(&dir);
    kuk_in(&dir).args(["add", "Test"]).assert().success();

    kuk_pm_in(&dir)
        .args(["pr", "1"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Not a git repository"));
}

#[test]
fn pr_nonexistent_card_fails() {
    let dir = TempDir::new().unwrap();
    init_git_and_kuk(&dir);

    kuk_pm_in(&dir)
        .args(["pr", "99"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Card not found"));
}

// ─── Commands before init ────────────────────────────────────

#[test]
fn branch_before_kuk_init_fails() {
    let dir = TempDir::new().unwrap();
    kuk_pm_in(&dir)
        .args(["branch", "1"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("kuk init"));
}

#[test]
fn sprint_before_init_fails() {
    let dir = TempDir::new().unwrap();
    kuk_pm_in(&dir)
        .args(["sprint", "list"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("kuk init"));
}
