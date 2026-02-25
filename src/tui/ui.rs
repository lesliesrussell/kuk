use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap};

use super::app::{App, Mode};

pub fn draw(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Title bar
            Constraint::Min(0),    // Board
            Constraint::Length(1), // Status bar
        ])
        .split(f.area());

    draw_title_bar(f, chunks[0], app);
    draw_board(f, chunks[1], app);
    draw_status_bar(f, chunks[2], app);

    if app.mode == Mode::Help {
        draw_help_overlay(f);
    }

    if app.mode == Mode::BoardPicker {
        draw_board_picker_overlay(f, app);
    }
}

fn draw_title_bar(f: &mut Frame, area: Rect, app: &App) {
    let title = format!(
        " kuk  │  {}  │  {} cards",
        app.board.name,
        app.board.cards.iter().filter(|c| !c.archived).count()
    );
    let bar = Paragraph::new(title).style(
        Style::default()
            .fg(Color::Black)
            .bg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    );
    f.render_widget(bar, area);
}

fn draw_board(f: &mut Frame, area: Rect, app: &App) {
    let num_cols = app.board.columns.len();
    if num_cols == 0 {
        return;
    }

    let constraints: Vec<Constraint> = (0..num_cols)
        .map(|_| Constraint::Ratio(1, num_cols as u32))
        .collect();

    let col_areas = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(constraints)
        .split(area);

    for (i, col) in app.board.columns.iter().enumerate() {
        let cards = app.column_cards(i);
        let is_selected_col = i == app.selected_col;

        let wip_info = col
            .wip_limit
            .map(|l| format!(" [{}/{}]", cards.len(), l))
            .unwrap_or_default();

        let header = format!("{} ({}){}", col.name.to_uppercase(), cards.len(), wip_info);

        let border_style = if is_selected_col {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default().fg(Color::DarkGray)
        };

        let block = Block::default()
            .title(header)
            .borders(Borders::ALL)
            .border_style(border_style);

        let items: Vec<ListItem> = cards
            .iter()
            .enumerate()
            .map(|(j, card)| {
                let is_selected = is_selected_col && j == app.selected_row;

                let labels = if card.labels.is_empty() {
                    String::new()
                } else {
                    format!(" [{}]", card.labels.join(","))
                };

                let assignee = card
                    .assignee
                    .as_ref()
                    .map(|a| format!(" @{a}"))
                    .unwrap_or_default();

                let text = format!("{}{}{}", card.title, labels, assignee);

                let style = if is_selected {
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::Cyan)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::White)
                };

                ListItem::new(Line::from(Span::styled(text, style)))
            })
            .collect();

        let list = List::new(items).block(block);
        f.render_widget(list, col_areas[i]);
    }
}

fn draw_status_bar(f: &mut Frame, area: Rect, app: &App) {
    let mode_str = match app.mode {
        Mode::Normal => "NORMAL",
        Mode::Insert => "INSERT",
        Mode::Search => "SEARCH",
        Mode::Help => "HELP",
        Mode::Confirm => "CONFIRM",
        Mode::BoardPicker => "BOARDS",
    };

    let left = match app.mode {
        Mode::Insert => format!(" {} │ {}", mode_str, app.input_buf),
        Mode::Search => format!(" {} │ /{}", mode_str, app.search_buf),
        _ => {
            if let Some(msg) = &app.message {
                format!(" {} │ {}", mode_str, msg)
            } else {
                format!(" {} │ ? for help", mode_str)
            }
        }
    };

    let bar = Paragraph::new(left).style(
        Style::default()
            .fg(Color::Black)
            .bg(Color::DarkGray)
            .add_modifier(Modifier::BOLD),
    );
    f.render_widget(bar, area);
}

fn draw_help_overlay(f: &mut Frame) {
    let area = centered_rect(60, 70, f.area());
    f.render_widget(Clear, area);

    let help_text = vec![
        Line::from(Span::styled(
            "  kuk — Keyboard Reference",
            Style::default()
                .add_modifier(Modifier::BOLD)
                .fg(Color::Cyan),
        )),
        Line::from(""),
        Line::from("  Navigation"),
        Line::from("    h/l or ←/→    Switch columns"),
        Line::from("    j/k or ↓/↑    Move up/down"),
        Line::from("    gg             Jump to top"),
        Line::from("    G              Jump to bottom"),
        Line::from(""),
        Line::from("  Actions"),
        Line::from("    a              Add card to current column"),
        Line::from("    d              Delete card (with confirm)"),
        Line::from("    x              Archive card"),
        Line::from("    L / >          Move card right"),
        Line::from("    H / <          Move card left"),
        Line::from("    K              Hoist (move to top)"),
        Line::from("    J              Demote (move to bottom)"),
        Line::from(""),
        Line::from("  Other"),
        Line::from("    b              Switch board"),
        Line::from("    /              Search"),
        Line::from("    r              Refresh board"),
        Line::from("    ?              Toggle help"),
        Line::from("    q / Ctrl+C     Quit"),
        Line::from(""),
        Line::from(Span::styled(
            "  Press Esc or ? to close",
            Style::default().fg(Color::DarkGray),
        )),
    ];

    let help = Paragraph::new(help_text)
        .block(
            Block::default()
                .title(" Help ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)),
        )
        .wrap(Wrap { trim: false })
        .alignment(Alignment::Left);

    f.render_widget(help, area);
}

fn draw_board_picker_overlay(f: &mut Frame, app: &App) {
    let height = (app.board_list.len() as u16 + 4).min(20);
    let width = 40u16;
    let area = centered_fixed(width, height, f.area());
    f.render_widget(Clear, area);

    let items: Vec<ListItem> = app
        .board_list
        .iter()
        .enumerate()
        .map(|(i, name)| {
            let is_active = *name == app.board.name;
            let is_selected = i == app.board_selected;
            let prefix = if is_active { "* " } else { "  " };
            let text = format!("{prefix}{name}");

            let style = if is_selected {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else if is_active {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };

            ListItem::new(Line::from(Span::styled(text, style)))
        })
        .collect();

    let list = List::new(items).block(
        Block::default()
            .title(" Switch Board ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan)),
    );

    f.render_widget(list, area);
}

fn centered_fixed(width: u16, height: u16, r: Rect) -> Rect {
    let x = r.x + r.width.saturating_sub(width) / 2;
    let y = r.y + r.height.saturating_sub(height) / 2;
    Rect::new(x, y, width.min(r.width), height.min(r.height))
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
