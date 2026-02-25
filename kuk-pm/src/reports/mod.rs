use chrono::{Datelike, Days, NaiveDate, Utc};
use serde::Serialize;

use kuk::model::Board;

use crate::model::Sprint;

// --- Column classification helpers ---

pub fn is_done_column(name: &str) -> bool {
    let lower = name.to_lowercase();
    lower == "done" || lower == "completed" || lower == "closed"
}

pub fn is_todo_column(name: &str) -> bool {
    let lower = name.to_lowercase();
    lower == "todo" || lower == "backlog" || lower == "to do"
}

pub fn is_wip_column(name: &str) -> bool {
    !is_done_column(name) && !is_todo_column(name)
}

fn week_start_monday(date: NaiveDate) -> NaiveDate {
    let days_from_monday = date.weekday().num_days_from_monday() as u64;
    date.checked_sub_days(Days::new(days_from_monday))
        .unwrap_or(date)
}

// ─── Velocity ────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct WeekBucket {
    pub week_start: NaiveDate,
    pub count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct VelocityReport {
    pub weeks: Vec<WeekBucket>,
    pub average: f64,
    pub trend: String,
}

pub fn calculate_velocity(boards: &[Board], num_weeks: u32) -> VelocityReport {
    let now = Utc::now().date_naive();
    let current_week = week_start_monday(now);

    // Build week boundaries going back num_weeks
    let week_starts: Vec<NaiveDate> = (0..num_weeks)
        .rev()
        .map(|i| {
            current_week
                .checked_sub_days(Days::new(i as u64 * 7))
                .unwrap_or(current_week)
        })
        .collect();

    // Collect done cards' completion dates
    let done_dates: Vec<NaiveDate> = boards
        .iter()
        .flat_map(|b| b.cards.iter())
        .filter(|c| !c.archived && is_done_column(&c.column))
        .map(|c| c.updated_at.date_naive())
        .collect();

    // Bucket done cards into weeks
    let weeks: Vec<WeekBucket> = week_starts
        .iter()
        .map(|&ws| {
            let we = ws.checked_add_days(Days::new(7)).unwrap_or(ws);
            let count = done_dates.iter().filter(|&&d| d >= ws && d < we).count();
            WeekBucket {
                week_start: ws,
                count,
            }
        })
        .collect();

    let total: usize = weeks.iter().map(|b| b.count).sum();
    let average = if num_weeks > 0 {
        total as f64 / num_weeks as f64
    } else {
        0.0
    };

    // Trend: compare first half vs second half
    let half = weeks.len() / 2;
    let first_half: usize = weeks[..half].iter().map(|b| b.count).sum();
    let second_half: usize = weeks[half..].iter().map(|b| b.count).sum();
    let trend = if second_half > first_half + 1 {
        "improving".into()
    } else if first_half > second_half + 1 {
        "declining".into()
    } else {
        "stable".into()
    };

    VelocityReport {
        weeks,
        average,
        trend,
    }
}

pub fn render_velocity_text(report: &VelocityReport) -> String {
    let mut out = String::new();
    out.push_str(&format!("Velocity (last {} weeks)\n", report.weeks.len()));
    out.push_str("────────────────────────────────\n");

    let max_count = report
        .weeks
        .iter()
        .map(|w| w.count)
        .max()
        .unwrap_or(1)
        .max(1);

    for week in &report.weeks {
        let bar_len = week.count * 20 / max_count;
        let bar: String = "█".repeat(bar_len);
        out.push_str(&format!(
            "  {}  {:>3}  {}\n",
            week.week_start, week.count, bar
        ));
    }

    out.push_str(&format!("\nAverage: {:.1} cards/week\n", report.average));
    let trend_arrow = match report.trend.as_str() {
        "improving" => "↑ improving",
        "declining" => "↓ declining",
        _ => "→ stable",
    };
    out.push_str(&format!("Trend: {trend_arrow}\n"));
    out
}

// ─── Burndown ────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct BurndownPoint {
    pub date: NaiveDate,
    pub ideal: f64,
    pub actual: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct BurndownReport {
    pub sprint_name: String,
    pub start: NaiveDate,
    pub end: NaiveDate,
    pub total_cards: usize,
    pub points: Vec<BurndownPoint>,
}

pub fn calculate_burndown(boards: &[Board], sprint: &Sprint) -> BurndownReport {
    let all_cards: Vec<_> = boards
        .iter()
        .flat_map(|b| b.cards.iter())
        .filter(|c| !c.archived)
        .collect();

    let total_cards = all_cards.len();
    let sprint_days = (sprint.end - sprint.start).num_days().max(1) as f64;
    let today = Utc::now().date_naive();

    // Generate points at weekly intervals
    let mut points = Vec::new();
    let mut date = sprint.start;
    loop {
        if date > sprint.end || date > today {
            break;
        }

        let day_offset = (date - sprint.start).num_days() as f64;
        let ideal = total_cards as f64 * (1.0 - day_offset / sprint_days);

        // Count cards done by this date (using updated_at as proxy)
        let done_by_date = all_cards
            .iter()
            .filter(|c| is_done_column(&c.column) && c.updated_at.date_naive() <= date)
            .count();
        let actual = total_cards.saturating_sub(done_by_date);

        points.push(BurndownPoint {
            date,
            ideal,
            actual,
        });

        date = match date.checked_add_days(Days::new(7)) {
            Some(d) => d,
            None => break,
        };
    }

    // Add final point at sprint end if past it
    if points.last().is_some_and(|p| p.date < sprint.end) && today >= sprint.end {
        let done = all_cards
            .iter()
            .filter(|c| is_done_column(&c.column))
            .count();
        points.push(BurndownPoint {
            date: sprint.end,
            ideal: 0.0,
            actual: total_cards.saturating_sub(done),
        });
    }

    BurndownReport {
        sprint_name: sprint.name.clone(),
        start: sprint.start,
        end: sprint.end,
        total_cards,
        points,
    }
}

pub fn render_burndown_text(report: &BurndownReport) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "Burndown: {} ({} → {})\n",
        report.sprint_name, report.start, report.end
    ));
    out.push_str("──────────────────────────────────────────────\n");
    out.push_str(&format!("Total scope: {} cards\n\n", report.total_cards));
    out.push_str("Date         Ideal  Actual  Remaining\n");

    for point in &report.points {
        let bar: String = "█".repeat(point.actual.min(30));
        out.push_str(&format!(
            "{}  {:>5.1}  {:>6}  {}\n",
            point.date, point.ideal, point.actual, bar
        ));
    }

    if let Some(last) = report.points.last() {
        out.push('\n');
        if last.actual == 0 {
            out.push_str("Status: Complete\n");
        } else if (last.actual as f64) <= last.ideal {
            out.push_str("Status: On track\n");
        } else {
            out.push_str(&format!(
                "Status: Behind schedule ({} remaining)\n",
                last.actual
            ));
        }
    }

    out
}

// ─── Roadmap ─────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct RoadmapWeek {
    pub week_start: NaiveDate,
    pub todo: usize,
    pub wip: usize,
    pub done: usize,
    pub milestones: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RoadmapReport {
    pub weeks: Vec<RoadmapWeek>,
    pub velocity: f64,
}

pub fn calculate_roadmap(
    boards: &[Board],
    sprints: &[Sprint],
    num_weeks: u32,
    velocity: f64,
) -> RoadmapReport {
    let now = Utc::now().date_naive();
    let current_week = week_start_monday(now);

    let all_cards: Vec<_> = boards
        .iter()
        .flat_map(|b| b.cards.iter())
        .filter(|c| !c.archived)
        .collect();

    let total_todo = all_cards
        .iter()
        .filter(|c| is_todo_column(&c.column))
        .count();
    let total_wip = all_cards
        .iter()
        .filter(|c| is_wip_column(&c.column))
        .count();
    let total_done = all_cards
        .iter()
        .filter(|c| is_done_column(&c.column))
        .count();

    let cards_per_week = velocity.max(0.1);

    let mut weeks = Vec::new();
    let mut remaining_todo = total_todo as f64;
    let mut remaining_wip = total_wip as f64;
    let mut projected_done = total_done as f64;

    for i in 0..num_weeks {
        let ws = current_week
            .checked_add_days(Days::new(i as u64 * 7))
            .unwrap_or(current_week);
        let we = ws.checked_add_days(Days::new(7)).unwrap_or(ws);

        // Find sprint milestones ending in this week
        let milestones: Vec<String> = sprints
            .iter()
            .filter(|s| s.end >= ws && s.end < we)
            .map(|s| format!("{} ends", s.name))
            .collect();

        weeks.push(RoadmapWeek {
            week_start: ws,
            todo: remaining_todo.round().max(0.0) as usize,
            wip: remaining_wip.round().max(0.0) as usize,
            done: projected_done.round() as usize,
            milestones,
        });

        // Project cards flowing through pipeline
        if i > 0 {
            let completed = cards_per_week.min(remaining_wip);
            remaining_wip -= completed;
            projected_done += completed;

            let started = cards_per_week.min(remaining_todo);
            remaining_todo -= started;
            remaining_wip += started;
        }
    }

    RoadmapReport { weeks, velocity }
}

pub fn render_roadmap_text(report: &RoadmapReport) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "Roadmap (next {} weeks, velocity: {:.1}/wk)\n",
        report.weeks.len(),
        report.velocity
    ));
    out.push_str("──────────────────────────────────────────────────\n");
    out.push_str("Week          Todo  Doing  Done  Milestones\n");

    for week in &report.weeks {
        let milestones = if week.milestones.is_empty() {
            String::new()
        } else {
            week.milestones.join(", ")
        };
        out.push_str(&format!(
            "{}  {:>4}  {:>5}  {:>4}  {}\n",
            week.week_start, week.todo, week.wip, week.done, milestones
        ));
    }

    let remaining = report.weeks.first().map(|w| w.todo + w.wip).unwrap_or(0);
    if remaining > 0 && report.velocity > 0.0 {
        let weeks_to_complete = (remaining as f64 / report.velocity).ceil() as u32;
        out.push_str(&format!(
            "\nEstimated completion: ~{weeks_to_complete} weeks ({remaining} cards remaining)\n"
        ));
    } else if remaining == 0 {
        out.push_str("\nAll work complete\n");
    }

    out
}

// ─── Stats ───────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct StatsReport {
    pub board_name: String,
    pub total_cards: usize,
    pub active_cards: usize,
    pub archived_cards: usize,
    pub wip_count: usize,
    pub wip_limit: Option<u32>,
    pub wip_violation: bool,
    pub done_7d: usize,
    pub done_30d: usize,
    pub avg_cycle_days: Option<f64>,
    pub oldest_wip: Option<(String, i64)>,
}

pub fn calculate_stats(board: &Board) -> StatsReport {
    let now = Utc::now();
    let cutoff_7d = now
        .date_naive()
        .checked_sub_days(Days::new(7))
        .unwrap_or(now.date_naive());
    let cutoff_30d = now
        .date_naive()
        .checked_sub_days(Days::new(30))
        .unwrap_or(now.date_naive());

    let active_cards: Vec<_> = board.cards.iter().filter(|c| !c.archived).collect();
    let archived_cards = board.cards.iter().filter(|c| c.archived).count();

    let wip_count = active_cards
        .iter()
        .filter(|c| is_wip_column(&c.column))
        .count();

    // Find WIP limit for middle columns
    let wip_limit = board
        .columns
        .iter()
        .filter(|col| is_wip_column(&col.name))
        .filter_map(|col| col.wip_limit)
        .next();

    let wip_violation = wip_limit.is_some_and(|limit| wip_count > limit as usize);

    // Throughput: done cards completed within window
    let done_7d = active_cards
        .iter()
        .filter(|c| is_done_column(&c.column) && c.updated_at.date_naive() >= cutoff_7d)
        .count();
    let done_30d = active_cards
        .iter()
        .filter(|c| is_done_column(&c.column) && c.updated_at.date_naive() >= cutoff_30d)
        .count();

    // Cycle time: avg(updated_at - created_at) for done cards
    let done_cards: Vec<_> = active_cards
        .iter()
        .filter(|c| is_done_column(&c.column))
        .collect();

    let avg_cycle_days = if done_cards.is_empty() {
        None
    } else {
        let total_hours: f64 = done_cards
            .iter()
            .map(|c| (c.updated_at - c.created_at).num_hours() as f64)
            .sum();
        Some(total_hours / done_cards.len() as f64 / 24.0)
    };

    // Oldest WIP card
    let oldest_wip = active_cards
        .iter()
        .filter(|c| is_wip_column(&c.column))
        .min_by_key(|c| c.created_at)
        .map(|c| {
            let days = (now - c.created_at).num_days();
            (c.title.clone(), days)
        });

    StatsReport {
        board_name: board.name.clone(),
        total_cards: board.cards.len(),
        active_cards: active_cards.len(),
        archived_cards,
        wip_count,
        wip_limit,
        wip_violation,
        done_7d,
        done_30d,
        avg_cycle_days,
        oldest_wip,
    }
}

pub fn render_stats_text(report: &StatsReport) -> String {
    let mut out = String::new();
    out.push_str("Project Statistics\n");
    out.push_str("──────────────────\n");
    out.push_str(&format!(
        "Board: {} ({} active, {} archived)\n\n",
        report.board_name, report.active_cards, report.archived_cards
    ));

    let wip_status = if report.wip_violation {
        format!("{} cards (OVER LIMIT)", report.wip_count)
    } else {
        format!("{} cards", report.wip_count)
    };
    out.push_str(&format!("Work in Progress:   {wip_status}\n"));

    match report.wip_limit {
        Some(limit) => out.push_str(&format!("WIP Limit:          {limit}\n")),
        None => out.push_str("WIP Limit:          none set\n"),
    }

    out.push_str(&format!("Throughput (7d):    {} cards\n", report.done_7d));
    out.push_str(&format!("Throughput (30d):   {} cards\n", report.done_30d));

    match report.avg_cycle_days {
        Some(days) => out.push_str(&format!("Avg Cycle Time:     {days:.1} days\n")),
        None => out.push_str("Avg Cycle Time:     no data\n"),
    }

    if let Some((ref title, days)) = report.oldest_wip {
        let display_title = if title.len() > 30 {
            &title[..30]
        } else {
            title
        };
        out.push_str(&format!(
            "Oldest WIP:         \"{display_title}\" ({days} days)\n"
        ));
    }

    out
}

// ─── Release Notes ───────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct ReleaseNotesReport {
    pub since: String,
    pub features: Vec<String>,
    pub fixes: Vec<String>,
    pub other: Vec<String>,
}

pub fn categorize_commits(commits: &[crate::git::CommitInfo]) -> ReleaseNotesReport {
    let mut features = Vec::new();
    let mut fixes = Vec::new();
    let mut other = Vec::new();

    for commit in commits {
        let msg = commit.message.trim();
        let first_line = msg.lines().next().unwrap_or(msg);

        if first_line.starts_with("feat") {
            features.push(first_line.to_string());
        } else if first_line.starts_with("fix") {
            fixes.push(first_line.to_string());
        } else {
            other.push(first_line.to_string());
        }
    }

    ReleaseNotesReport {
        since: String::new(),
        features,
        fixes,
        other,
    }
}

pub fn render_release_notes_text(report: &ReleaseNotesReport) -> String {
    let mut out = String::new();
    out.push_str(&format!("Release Notes (since {})\n", report.since));
    out.push_str("════════════════════════════════════════\n\n");

    if !report.features.is_empty() {
        out.push_str("Features\n");
        out.push_str("────────\n");
        for f in &report.features {
            out.push_str(&format!("  - {f}\n"));
        }
        out.push('\n');
    }

    if !report.fixes.is_empty() {
        out.push_str("Fixes\n");
        out.push_str("─────\n");
        for f in &report.fixes {
            out.push_str(&format!("  - {f}\n"));
        }
        out.push('\n');
    }

    if !report.other.is_empty() {
        out.push_str("Other\n");
        out.push_str("─────\n");
        for o in &report.other {
            out.push_str(&format!("  - {o}\n"));
        }
        out.push('\n');
    }

    let total = report.features.len() + report.fixes.len() + report.other.len();
    out.push_str(&format!("{total} commits total\n"));
    out
}

// ─── Tests ───────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use kuk::model::{Board, Card, Column};

    fn make_board_with_cards() -> Board {
        let now = Utc::now();
        let mut board = Board {
            name: "test".into(),
            columns: vec![
                Column {
                    name: "todo".into(),
                    wip_limit: None,
                },
                Column {
                    name: "doing".into(),
                    wip_limit: Some(3),
                },
                Column {
                    name: "done".into(),
                    wip_limit: None,
                },
            ],
            cards: Vec::new(),
        };

        let mut c1 = Card::new("Task A", "todo");
        c1.order = 0;
        board.cards.push(c1);

        let mut c2 = Card::new("Task B", "doing");
        c2.order = 0;
        board.cards.push(c2);

        let mut c3 = Card::new("Task C", "done");
        c3.order = 0;
        c3.updated_at = now - chrono::TimeDelta::try_days(2).expect("valid delta");
        board.cards.push(c3);

        let mut c4 = Card::new("Task D", "done");
        c4.order = 1;
        c4.updated_at = now - chrono::TimeDelta::try_days(5).expect("valid delta");
        board.cards.push(c4);

        board
    }

    #[test]
    fn test_is_done_column() {
        assert!(is_done_column("done"));
        assert!(is_done_column("Done"));
        assert!(is_done_column("DONE"));
        assert!(is_done_column("completed"));
        assert!(is_done_column("closed"));
        assert!(!is_done_column("doing"));
        assert!(!is_done_column("todo"));
    }

    #[test]
    fn test_is_todo_column() {
        assert!(is_todo_column("todo"));
        assert!(is_todo_column("backlog"));
        assert!(is_todo_column("to do"));
        assert!(!is_todo_column("doing"));
        assert!(!is_todo_column("done"));
    }

    #[test]
    fn test_is_wip_column() {
        assert!(is_wip_column("doing"));
        assert!(is_wip_column("in progress"));
        assert!(is_wip_column("review"));
        assert!(!is_wip_column("todo"));
        assert!(!is_wip_column("done"));
    }

    #[test]
    fn test_week_start_monday() {
        // 2026-02-25 is a Wednesday
        let wed = NaiveDate::from_ymd_opt(2026, 2, 25).unwrap();
        let mon = week_start_monday(wed);
        assert_eq!(mon, NaiveDate::from_ymd_opt(2026, 2, 23).unwrap());

        // Monday stays Monday
        let already_mon = NaiveDate::from_ymd_opt(2026, 2, 23).unwrap();
        assert_eq!(week_start_monday(already_mon), already_mon);
    }

    #[test]
    fn test_velocity_with_done_cards() {
        let board = make_board_with_cards();
        let report = calculate_velocity(&[board], 4);
        assert_eq!(report.weeks.len(), 4);
        assert!(report.average >= 0.0);
    }

    #[test]
    fn test_velocity_empty_board() {
        let board = Board::default_board();
        let report = calculate_velocity(&[board], 4);
        assert_eq!(report.weeks.len(), 4);
        assert_eq!(report.average, 0.0);
        assert_eq!(report.trend, "stable");
    }

    #[test]
    fn test_velocity_render_contains_headers() {
        let board = make_board_with_cards();
        let report = calculate_velocity(&[board], 4);
        let text = render_velocity_text(&report);
        assert!(text.contains("Velocity"));
        assert!(text.contains("Average"));
        assert!(text.contains("Trend"));
    }

    #[test]
    fn test_stats_basic() {
        let board = make_board_with_cards();
        let stats = calculate_stats(&board);
        assert_eq!(stats.board_name, "test");
        assert_eq!(stats.active_cards, 4);
        assert_eq!(stats.wip_count, 1);
        assert!(!stats.wip_violation);
    }

    #[test]
    fn test_stats_wip_violation() {
        let mut board = make_board_with_cards();
        for i in 0..4 {
            let mut c = Card::new(&format!("Extra {i}"), "doing");
            c.order = (i + 1) as u32;
            board.cards.push(c);
        }
        let stats = calculate_stats(&board);
        assert_eq!(stats.wip_count, 5);
        assert!(stats.wip_violation);
    }

    #[test]
    fn test_stats_cycle_time() {
        let board = make_board_with_cards();
        let stats = calculate_stats(&board);
        assert!(stats.avg_cycle_days.is_some());
    }

    #[test]
    fn test_stats_render() {
        let board = make_board_with_cards();
        let stats = calculate_stats(&board);
        let text = render_stats_text(&stats);
        assert!(text.contains("Project Statistics"));
        assert!(text.contains("Work in Progress"));
        assert!(text.contains("Throughput"));
    }

    #[test]
    fn test_burndown_basic() {
        let board = make_board_with_cards();
        let sprint = Sprint {
            name: "test-sprint".into(),
            start: Utc::now()
                .date_naive()
                .checked_sub_days(Days::new(14))
                .unwrap(),
            end: Utc::now()
                .date_naive()
                .checked_add_days(Days::new(14))
                .unwrap(),
            goal: None,
            boards: vec!["test".into()],
            status: crate::model::SprintStatus::Active,
        };
        let report = calculate_burndown(&[board], &sprint);
        assert_eq!(report.sprint_name, "test-sprint");
        assert_eq!(report.total_cards, 4);
        assert!(!report.points.is_empty());
    }

    #[test]
    fn test_burndown_render() {
        let board = make_board_with_cards();
        let sprint = Sprint {
            name: "test-sprint".into(),
            start: Utc::now()
                .date_naive()
                .checked_sub_days(Days::new(7))
                .unwrap(),
            end: Utc::now()
                .date_naive()
                .checked_add_days(Days::new(7))
                .unwrap(),
            goal: None,
            boards: vec!["test".into()],
            status: crate::model::SprintStatus::Active,
        };
        let report = calculate_burndown(&[board], &sprint);
        let text = render_burndown_text(&report);
        assert!(text.contains("Burndown: test-sprint"));
        assert!(text.contains("Total scope"));
    }

    #[test]
    fn test_roadmap_basic() {
        let board = make_board_with_cards();
        let report = calculate_roadmap(&[board], &[], 8, 2.0);
        assert_eq!(report.weeks.len(), 8);
        assert_eq!(report.velocity, 2.0);
        assert_eq!(report.weeks[0].todo, 1);
        assert_eq!(report.weeks[0].wip, 1);
        assert_eq!(report.weeks[0].done, 2);
    }

    #[test]
    fn test_roadmap_render() {
        let board = make_board_with_cards();
        let report = calculate_roadmap(&[board], &[], 8, 2.0);
        let text = render_roadmap_text(&report);
        assert!(text.contains("Roadmap"));
        assert!(text.contains("Todo"));
        assert!(text.contains("Doing"));
        assert!(text.contains("Done"));
    }

    #[test]
    fn test_roadmap_with_sprint_milestones() {
        let board = make_board_with_cards();
        let sprint = Sprint {
            name: "sprint-1".into(),
            start: Utc::now().date_naive(),
            end: Utc::now()
                .date_naive()
                .checked_add_days(Days::new(3))
                .unwrap(),
            goal: None,
            boards: Vec::new(),
            status: crate::model::SprintStatus::Active,
        };
        let report = calculate_roadmap(&[board], &[sprint], 4, 1.0);
        let has_milestone = report.weeks.iter().any(|w| !w.milestones.is_empty());
        assert!(has_milestone);
    }

    #[test]
    fn test_categorize_commits() {
        let commits = vec![
            crate::git::CommitInfo {
                sha: "abc".into(),
                message: "feat: add login".into(),
                author: "dev".into(),
                time: 0,
            },
            crate::git::CommitInfo {
                sha: "def".into(),
                message: "fix: null pointer".into(),
                author: "dev".into(),
                time: 0,
            },
            crate::git::CommitInfo {
                sha: "ghi".into(),
                message: "chore: update deps".into(),
                author: "dev".into(),
                time: 0,
            },
        ];
        let report = categorize_commits(&commits);
        assert_eq!(report.features.len(), 1);
        assert_eq!(report.fixes.len(), 1);
        assert_eq!(report.other.len(), 1);
    }

    #[test]
    fn test_release_notes_render() {
        let report = ReleaseNotesReport {
            since: "v0.1.0".into(),
            features: vec!["feat: add login".into()],
            fixes: vec!["fix: null pointer".into()],
            other: vec!["chore: update deps".into()],
        };
        let text = render_release_notes_text(&report);
        assert!(text.contains("Release Notes"));
        assert!(text.contains("Features"));
        assert!(text.contains("Fixes"));
        assert!(text.contains("3 commits total"));
    }
}
