use std::io;
use std::path::Path;
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;

use crate::error::{KukError, Result};
use crate::model::{Board, Card};
use crate::storage::Store;

use super::ui;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Mode {
    Normal,
    Insert,
    Search,
    Help,
    Confirm,
    BoardPicker,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ConfirmAction {
    Delete,
}

pub struct App {
    pub store: Store,
    pub board: Board,
    pub mode: Mode,
    pub selected_col: usize,
    pub selected_row: usize,
    pub input_buf: String,
    pub search_buf: String,
    pub search_active: bool,
    pub message: Option<String>,
    pub should_quit: bool,
    pub pending_confirm: Option<ConfirmAction>,
    pub pending_g: bool,
    pub board_list: Vec<String>,
    pub board_selected: usize,
}

impl App {
    pub fn new(repo_root: &Path) -> Result<Self> {
        let store = Store::new(repo_root);
        if !store.is_initialized() {
            return Err(KukError::NotInitialized);
        }
        let config = store.load_config()?;
        let board = store.load_board(&config.default_board)?;

        Ok(Self {
            store,
            board,
            mode: Mode::Normal,
            selected_col: 0,
            selected_row: 0,
            input_buf: String::new(),
            search_buf: String::new(),
            search_active: false,
            message: None,
            should_quit: false,
            pending_confirm: None,
            pending_g: false,
            board_list: Vec::new(),
            board_selected: 0,
        })
    }

    pub fn reload_board(&mut self) -> Result<()> {
        let config = self.store.load_config()?;
        self.board = self.store.load_board(&config.default_board)?;
        Ok(())
    }

    pub fn save_board(&self) -> Result<()> {
        self.store.save_board(&self.board)
    }

    /// Get active (non-archived) cards for a column, sorted by order.
    pub fn column_cards(&self, col_idx: usize) -> Vec<&Card> {
        if col_idx >= self.board.columns.len() {
            return Vec::new();
        }
        let col_name = &self.board.columns[col_idx].name;
        let mut cards: Vec<&Card> = self
            .board
            .cards
            .iter()
            .filter(|c| &c.column == col_name && !c.archived)
            .collect();
        cards.sort_by_key(|c| c.order);

        if self.search_active && !self.search_buf.is_empty() {
            let query = self.search_buf.to_lowercase();
            cards.retain(|c| c.title.to_lowercase().contains(&query));
        }
        cards
    }

    pub fn current_card(&self) -> Option<&Card> {
        let cards = self.column_cards(self.selected_col);
        cards.get(self.selected_row).copied()
    }

    pub fn current_card_id(&self) -> Option<String> {
        self.current_card().map(|c| c.id.clone())
    }

    fn clamp_row(&mut self) {
        let count = self.column_cards(self.selected_col).len();
        if count == 0 {
            self.selected_row = 0;
        } else if self.selected_row >= count {
            self.selected_row = count - 1;
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent) {
        // Ctrl+C always quits
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            self.should_quit = true;
            return;
        }

        match self.mode {
            Mode::Normal => self.handle_normal(key),
            Mode::Insert => self.handle_insert(key),
            Mode::Search => self.handle_search(key),
            Mode::Help => self.handle_help(key),
            Mode::Confirm => self.handle_confirm(key),
            Mode::BoardPicker => self.handle_board_picker(key),
        }
    }

    fn handle_normal(&mut self, key: KeyEvent) {
        match key.code {
            // Quit
            KeyCode::Char('q') => self.should_quit = true,

            // Navigation
            KeyCode::Char('j') | KeyCode::Down => {
                self.pending_g = false;
                let count = self.column_cards(self.selected_col).len();
                if count > 0 && self.selected_row < count - 1 {
                    self.selected_row += 1;
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.pending_g = false;
                if self.selected_row > 0 {
                    self.selected_row -= 1;
                }
            }
            KeyCode::Char('h') | KeyCode::Left => {
                self.pending_g = false;
                if self.selected_col > 0 {
                    self.selected_col -= 1;
                    self.clamp_row();
                }
            }
            KeyCode::Char('l') | KeyCode::Right => {
                self.pending_g = false;
                if self.selected_col < self.board.columns.len() - 1 {
                    self.selected_col += 1;
                    self.clamp_row();
                }
            }

            // gg = top, G = bottom
            KeyCode::Char('g') => {
                if self.pending_g {
                    self.selected_row = 0;
                    self.pending_g = false;
                } else {
                    self.pending_g = true;
                }
            }
            KeyCode::Char('G') => {
                self.pending_g = false;
                let count = self.column_cards(self.selected_col).len();
                if count > 0 {
                    self.selected_row = count - 1;
                }
            }

            // Add card
            KeyCode::Char('a') => {
                self.pending_g = false;
                self.mode = Mode::Insert;
                self.input_buf.clear();
                self.message = Some("Add card (Enter to save, Esc to cancel):".into());
            }

            // Delete card (dd)
            KeyCode::Char('d') => {
                if self.pending_g {
                    // "gd" is not a thing, reset
                    self.pending_g = false;
                    return;
                }
                // For dd: use confirm mode
                if self.current_card().is_some() {
                    self.mode = Mode::Confirm;
                    self.pending_confirm = Some(ConfirmAction::Delete);
                    self.message = Some("Delete this card? (y/n)".into());
                }
            }

            // Move card right (to next column)
            KeyCode::Char('>') | KeyCode::Char('L') => {
                self.pending_g = false;
                self.move_card_right();
            }

            // Move card left (to previous column)
            KeyCode::Char('<') | KeyCode::Char('H') => {
                self.pending_g = false;
                self.move_card_left();
            }

            // Hoist (move to top)
            KeyCode::Char('K') => {
                self.pending_g = false;
                self.hoist_card();
            }

            // Demote (move to bottom)
            KeyCode::Char('J') => {
                self.pending_g = false;
                self.demote_card();
            }

            // Archive
            KeyCode::Char('x') => {
                self.pending_g = false;
                self.archive_card();
            }

            // Search
            KeyCode::Char('/') => {
                self.pending_g = false;
                self.mode = Mode::Search;
                self.search_buf.clear();
                self.search_active = true;
                self.message = Some("Search:".into());
            }

            // Clear search
            KeyCode::Esc => {
                self.pending_g = false;
                self.search_active = false;
                self.search_buf.clear();
                self.message = None;
                self.clamp_row();
            }

            // Help
            KeyCode::Char('?') => {
                self.pending_g = false;
                self.mode = Mode::Help;
            }

            // Refresh
            KeyCode::Char('r') => {
                self.pending_g = false;
                if let Err(e) = self.reload_board() {
                    self.message = Some(format!("Reload failed: {e}"));
                } else {
                    self.message = Some("Board reloaded.".into());
                    self.clamp_row();
                }
            }

            // Board picker
            KeyCode::Char('b') => {
                self.pending_g = false;
                self.open_board_picker();
            }

            _ => {
                self.pending_g = false;
            }
        }
    }

    fn handle_insert(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.mode = Mode::Normal;
                self.input_buf.clear();
                self.message = None;
            }
            KeyCode::Enter => {
                if !self.input_buf.is_empty() {
                    let col_name = self.board.columns[self.selected_col].name.clone();
                    let mut card = Card::new(&self.input_buf, &col_name);
                    card.order = self.board.next_order(&col_name);
                    self.board.cards.push(card);
                    if let Err(e) = self.save_board() {
                        self.message = Some(format!("Save failed: {e}"));
                    } else {
                        self.message = Some(format!("Added: {}", self.input_buf));
                        self.selected_row = self.column_cards(self.selected_col).len() - 1;
                    }
                }
                self.input_buf.clear();
                self.mode = Mode::Normal;
            }
            KeyCode::Backspace => {
                self.input_buf.pop();
            }
            KeyCode::Char(c) => {
                self.input_buf.push(c);
            }
            _ => {}
        }
    }

    fn handle_search(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.mode = Mode::Normal;
                self.search_active = false;
                self.search_buf.clear();
                self.message = None;
                self.clamp_row();
            }
            KeyCode::Enter => {
                self.mode = Mode::Normal;
                self.message = None;
                self.selected_row = 0;
            }
            KeyCode::Backspace => {
                self.search_buf.pop();
                self.selected_row = 0;
            }
            KeyCode::Char(c) => {
                self.search_buf.push(c);
                self.selected_row = 0;
            }
            _ => {}
        }
    }

    fn handle_help(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('?') => {
                self.mode = Mode::Normal;
            }
            _ => {}
        }
    }

    fn handle_confirm(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                if let Some(action) = self.pending_confirm.take() {
                    match action {
                        ConfirmAction::Delete => self.delete_current_card(),
                    }
                }
                self.mode = Mode::Normal;
                self.message = None;
            }
            _ => {
                self.pending_confirm = None;
                self.mode = Mode::Normal;
                self.message = None;
            }
        }
    }

    fn open_board_picker(&mut self) {
        match self.store.list_boards() {
            Ok(boards) => {
                // Find the index of the current board
                let current = boards
                    .iter()
                    .position(|b| *b == self.board.name)
                    .unwrap_or(0);
                self.board_list = boards;
                self.board_selected = current;
                self.mode = Mode::BoardPicker;
                self.message = Some("Switch board (Enter to select, Esc to cancel):".into());
            }
            Err(e) => {
                self.message = Some(format!("Failed to list boards: {e}"));
            }
        }
    }

    fn handle_board_picker(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.mode = Mode::Normal;
                self.message = None;
            }
            KeyCode::Char('j') | KeyCode::Down => {
                if !self.board_list.is_empty() && self.board_selected < self.board_list.len() - 1 {
                    self.board_selected += 1;
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if self.board_selected > 0 {
                    self.board_selected -= 1;
                }
            }
            KeyCode::Enter => {
                if let Some(name) = self.board_list.get(self.board_selected).cloned() {
                    if name == self.board.name {
                        // Already on this board
                        self.mode = Mode::Normal;
                        self.message = None;
                        return;
                    }
                    // Switch: save config, reload board
                    match self.store.load_config() {
                        Ok(mut config) => {
                            config.default_board = name.clone();
                            if let Err(e) = self.store.save_config(&config) {
                                self.message = Some(format!("Save config failed: {e}"));
                                self.mode = Mode::Normal;
                                return;
                            }
                            match self.store.load_board(&name) {
                                Ok(board) => {
                                    self.board = board;
                                    self.selected_col = 0;
                                    self.selected_row = 0;
                                    self.search_active = false;
                                    self.search_buf.clear();
                                    self.message = Some(format!("Switched to board: {name}"));
                                }
                                Err(e) => {
                                    self.message = Some(format!("Load board failed: {e}"));
                                }
                            }
                        }
                        Err(e) => {
                            self.message = Some(format!("Load config failed: {e}"));
                        }
                    }
                }
                self.mode = Mode::Normal;
            }
            KeyCode::Char('q') => {
                self.mode = Mode::Normal;
                self.message = None;
            }
            _ => {}
        }
    }

    fn move_card_right(&mut self) {
        let next_col = self.selected_col + 1;
        if next_col >= self.board.columns.len() {
            return;
        }
        if let Some(id) = self.current_card_id() {
            let to = self.board.columns[next_col].name.clone();
            let order = self.board.next_order(&to);
            if let Some(card) = self.board.find_card_mut(&id) {
                card.column = to;
                card.order = order;
                card.updated_at = chrono::Utc::now();
                let _ = self.save_board();
                self.message = Some(format!("Moved → {}", self.board.columns[next_col].name));
                self.clamp_row();
            }
        }
    }

    fn move_card_left(&mut self) {
        if self.selected_col == 0 {
            return;
        }
        let prev_col = self.selected_col - 1;
        if let Some(id) = self.current_card_id() {
            let to = self.board.columns[prev_col].name.clone();
            let order = self.board.next_order(&to);
            if let Some(card) = self.board.find_card_mut(&id) {
                card.column = to;
                card.order = order;
                card.updated_at = chrono::Utc::now();
                let _ = self.save_board();
                self.message = Some(format!("Moved → {}", self.board.columns[prev_col].name));
                self.clamp_row();
            }
        }
    }

    fn hoist_card(&mut self) {
        if let Some(id) = self.current_card_id() {
            let column = self.board.find_card(&id).unwrap().column.clone();
            for c in self.board.cards.iter_mut() {
                if c.column == column && !c.archived && c.id != id {
                    c.order += 1;
                }
            }
            if let Some(card) = self.board.find_card_mut(&id) {
                card.order = 0;
                card.updated_at = chrono::Utc::now();
            }
            let _ = self.save_board();
            self.selected_row = 0;
            self.message = Some("Hoisted to top.".into());
        }
    }

    fn demote_card(&mut self) {
        if let Some(id) = self.current_card_id() {
            let column = self.board.find_card(&id).unwrap().column.clone();
            let max_order = self.board.next_order(&column);
            if let Some(card) = self.board.find_card_mut(&id) {
                card.order = max_order;
                card.updated_at = chrono::Utc::now();
            }
            let _ = self.save_board();
            let count = self.column_cards(self.selected_col).len();
            if count > 0 {
                self.selected_row = count - 1;
            }
            self.message = Some("Demoted to bottom.".into());
        }
    }

    fn archive_card(&mut self) {
        if let Some(id) = self.current_card_id() {
            if let Some(card) = self.board.find_card_mut(&id) {
                card.archived = true;
                card.updated_at = chrono::Utc::now();
                self.message = Some(format!("Archived: {}", card.title));
            }
            let _ = self.save_board();
            self.clamp_row();
        }
    }

    fn delete_current_card(&mut self) {
        if let Some(id) = self.current_card_id() {
            let title = self
                .board
                .find_card(&id)
                .map(|c| c.title.clone())
                .unwrap_or_default();
            self.board.cards.retain(|c| c.id != id);
            let _ = self.save_board();
            self.clamp_row();
            self.message = Some(format!("Deleted: {title}"));
        }
    }
}

pub fn run_tui(repo_root: &Path) -> Result<()> {
    let mut app = App::new(repo_root)?;

    enable_raw_mode().map_err(|e| KukError::Other(format!("Terminal error: {e}")))?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)
        .map_err(|e| KukError::Other(format!("Terminal error: {e}")))?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal =
        Terminal::new(backend).map_err(|e| KukError::Other(format!("Terminal error: {e}")))?;

    let result = run_loop(&mut terminal, &mut app);

    // Restore terminal
    disable_raw_mode().ok();
    execute!(terminal.backend_mut(), LeaveAlternateScreen).ok();
    terminal.show_cursor().ok();

    result
}

fn run_loop(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>, app: &mut App) -> Result<()> {
    loop {
        terminal
            .draw(|f| ui::draw(f, app))
            .map_err(|e| KukError::Other(format!("Draw error: {e}")))?;

        if event::poll(Duration::from_millis(100))
            .map_err(|e| KukError::Other(format!("Event error: {e}")))?
            && let Event::Key(key) =
                event::read().map_err(|e| KukError::Other(format!("Event error: {e}")))?
        {
            app.handle_key(key);
        }

        if app.should_quit {
            return Ok(());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};
    use tempfile::TempDir;

    fn make_key(code: KeyCode) -> KeyEvent {
        KeyEvent {
            code,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }
    }

    fn make_key_mod(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
        KeyEvent {
            code,
            modifiers,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }
    }

    fn make_shift_key(code: KeyCode) -> KeyEvent {
        make_key_mod(code, KeyModifiers::SHIFT)
    }

    fn test_app() -> (TempDir, App) {
        let dir = TempDir::new().unwrap();
        let store = Store::new(dir.path());
        store.init().unwrap();

        // Add some test cards
        let mut board = store.load_board("default").unwrap();
        let mut c1 = Card::new("Task A", "todo");
        c1.order = 0;
        let mut c2 = Card::new("Task B", "todo");
        c2.order = 1;
        let mut c3 = Card::new("Task C", "doing");
        c3.order = 0;
        board.cards.push(c1);
        board.cards.push(c2);
        board.cards.push(c3);
        store.save_board(&board).unwrap();

        let app = App::new(dir.path()).unwrap();
        (dir, app)
    }

    #[test]
    fn app_initializes() {
        let (_dir, app) = test_app();
        assert_eq!(app.mode, Mode::Normal);
        assert_eq!(app.selected_col, 0);
        assert_eq!(app.selected_row, 0);
        assert!(!app.should_quit);
    }

    #[test]
    fn app_not_initialized_fails() {
        let dir = TempDir::new().unwrap();
        let result = App::new(dir.path());
        assert!(result.is_err());
    }

    #[test]
    fn column_cards_returns_sorted() {
        let (_dir, app) = test_app();
        let cards = app.column_cards(0);
        assert_eq!(cards.len(), 2);
        assert_eq!(cards[0].title, "Task A");
        assert_eq!(cards[1].title, "Task B");
    }

    #[test]
    fn nav_j_moves_down() {
        let (_dir, mut app) = test_app();
        assert_eq!(app.selected_row, 0);
        app.handle_key(make_key(KeyCode::Char('j')));
        assert_eq!(app.selected_row, 1);
    }

    #[test]
    fn nav_k_moves_up() {
        let (_dir, mut app) = test_app();
        app.selected_row = 1;
        app.handle_key(make_key(KeyCode::Char('k')));
        assert_eq!(app.selected_row, 0);
    }

    #[test]
    fn nav_k_at_top_stays() {
        let (_dir, mut app) = test_app();
        app.handle_key(make_key(KeyCode::Char('k')));
        assert_eq!(app.selected_row, 0);
    }

    #[test]
    fn nav_j_at_bottom_stays() {
        let (_dir, mut app) = test_app();
        app.handle_key(make_key(KeyCode::Char('j')));
        app.handle_key(make_key(KeyCode::Char('j')));
        app.handle_key(make_key(KeyCode::Char('j')));
        assert_eq!(app.selected_row, 1); // only 2 items in todo
    }

    #[test]
    fn nav_l_moves_right() {
        let (_dir, mut app) = test_app();
        app.handle_key(make_key(KeyCode::Char('l')));
        assert_eq!(app.selected_col, 1);
    }

    #[test]
    fn nav_h_moves_left() {
        let (_dir, mut app) = test_app();
        app.selected_col = 1;
        app.handle_key(make_key(KeyCode::Char('h')));
        assert_eq!(app.selected_col, 0);
    }

    #[test]
    fn nav_h_at_left_stays() {
        let (_dir, mut app) = test_app();
        app.handle_key(make_key(KeyCode::Char('h')));
        assert_eq!(app.selected_col, 0);
    }

    #[test]
    fn nav_l_at_right_stays() {
        let (_dir, mut app) = test_app();
        app.handle_key(make_key(KeyCode::Char('l')));
        app.handle_key(make_key(KeyCode::Char('l')));
        app.handle_key(make_key(KeyCode::Char('l')));
        app.handle_key(make_key(KeyCode::Char('l')));
        assert_eq!(app.selected_col, 2); // 3 columns
    }

    #[test]
    fn nav_gg_goes_to_top() {
        let (_dir, mut app) = test_app();
        app.selected_row = 1;
        app.handle_key(make_key(KeyCode::Char('g')));
        app.handle_key(make_key(KeyCode::Char('g')));
        assert_eq!(app.selected_row, 0);
    }

    #[test]
    fn nav_big_g_goes_to_bottom() {
        let (_dir, mut app) = test_app();
        app.handle_key(make_key(KeyCode::Char('G')));
        assert_eq!(app.selected_row, 1); // 2 items, last is index 1
    }

    #[test]
    fn quit_on_q() {
        let (_dir, mut app) = test_app();
        app.handle_key(make_key(KeyCode::Char('q')));
        assert!(app.should_quit);
    }

    #[test]
    fn quit_on_ctrl_c() {
        let (_dir, mut app) = test_app();
        app.handle_key(make_key_mod(KeyCode::Char('c'), KeyModifiers::CONTROL));
        assert!(app.should_quit);
    }

    #[test]
    fn insert_mode_add_card() {
        let (_dir, mut app) = test_app();
        // Press 'a' to enter insert mode
        app.handle_key(make_key(KeyCode::Char('a')));
        assert_eq!(app.mode, Mode::Insert);

        // Type title
        app.handle_key(make_key(KeyCode::Char('N')));
        app.handle_key(make_key(KeyCode::Char('e')));
        app.handle_key(make_key(KeyCode::Char('w')));
        assert_eq!(app.input_buf, "New");

        // Press Enter to save
        app.handle_key(make_key(KeyCode::Enter));
        assert_eq!(app.mode, Mode::Normal);

        // Should now have 3 cards in todo
        assert_eq!(app.column_cards(0).len(), 3);
    }

    #[test]
    fn insert_mode_cancel() {
        let (_dir, mut app) = test_app();
        app.handle_key(make_key(KeyCode::Char('a')));
        app.handle_key(make_key(KeyCode::Char('X')));
        app.handle_key(make_key(KeyCode::Esc));
        assert_eq!(app.mode, Mode::Normal);
        assert_eq!(app.column_cards(0).len(), 2); // unchanged
    }

    #[test]
    fn insert_mode_backspace() {
        let (_dir, mut app) = test_app();
        app.handle_key(make_key(KeyCode::Char('a')));
        app.handle_key(make_key(KeyCode::Char('A')));
        app.handle_key(make_key(KeyCode::Char('B')));
        app.handle_key(make_key(KeyCode::Backspace));
        assert_eq!(app.input_buf, "A");
    }

    #[test]
    fn search_filters_cards() {
        let (_dir, mut app) = test_app();
        // Enter search mode
        app.handle_key(make_key(KeyCode::Char('/')));
        assert_eq!(app.mode, Mode::Search);

        // Type search query — "k A" uniquely matches "Task A" but not "Task B"
        app.handle_key(make_key(KeyCode::Char('k')));
        app.handle_key(make_key(KeyCode::Char(' ')));
        app.handle_key(make_key(KeyCode::Char('A')));
        app.handle_key(make_key(KeyCode::Enter));

        // Search active: only "Task A" matches
        assert!(app.search_active);
        let cards = app.column_cards(0);
        assert_eq!(cards.len(), 1);
        assert_eq!(cards[0].title, "Task A");
    }

    #[test]
    fn search_esc_clears() {
        let (_dir, mut app) = test_app();
        app.handle_key(make_key(KeyCode::Char('/')));
        app.handle_key(make_key(KeyCode::Char('Z')));
        app.handle_key(make_key(KeyCode::Esc));
        assert!(!app.search_active);
        assert_eq!(app.column_cards(0).len(), 2);
    }

    #[test]
    fn move_card_right_shift_l() {
        let (_dir, mut app) = test_app();
        assert_eq!(app.column_cards(0).len(), 2);
        assert_eq!(app.column_cards(1).len(), 1);

        // Move first todo card to doing
        app.handle_key(make_shift_key(KeyCode::Char('L')));

        assert_eq!(app.column_cards(0).len(), 1);
        assert_eq!(app.column_cards(1).len(), 2);
    }

    #[test]
    fn move_card_left_shift_h() {
        let (_dir, mut app) = test_app();
        // Go to doing column
        app.handle_key(make_key(KeyCode::Char('l')));
        assert_eq!(app.selected_col, 1);
        assert_eq!(app.column_cards(1).len(), 1);

        // Move card left
        app.handle_key(make_shift_key(KeyCode::Char('H')));
        assert_eq!(app.column_cards(0).len(), 3);
        assert_eq!(app.column_cards(1).len(), 0);
    }

    #[test]
    fn hoist_card_shift_k() {
        let (_dir, mut app) = test_app();
        // Select second card
        app.handle_key(make_key(KeyCode::Char('j')));
        assert_eq!(app.selected_row, 1);

        // Hoist it
        app.handle_key(make_shift_key(KeyCode::Char('K')));
        assert_eq!(app.selected_row, 0);
        let cards = app.column_cards(0);
        assert_eq!(cards[0].title, "Task B");
    }

    #[test]
    fn demote_card_shift_j() {
        let (_dir, mut app) = test_app();
        // Demote first card
        app.handle_key(make_shift_key(KeyCode::Char('J')));
        let cards = app.column_cards(0);
        assert_eq!(cards[0].title, "Task B");
        assert_eq!(cards[1].title, "Task A");
    }

    #[test]
    fn archive_card_x() {
        let (_dir, mut app) = test_app();
        assert_eq!(app.column_cards(0).len(), 2);
        app.handle_key(make_key(KeyCode::Char('x')));
        assert_eq!(app.column_cards(0).len(), 1);
    }

    #[test]
    fn delete_card_confirm_y() {
        let (_dir, mut app) = test_app();
        assert_eq!(app.column_cards(0).len(), 2);
        // Press 'd' to initiate delete
        app.handle_key(make_key(KeyCode::Char('d')));
        assert_eq!(app.mode, Mode::Confirm);

        // Confirm with 'y'
        app.handle_key(make_key(KeyCode::Char('y')));
        assert_eq!(app.mode, Mode::Normal);
        assert_eq!(app.column_cards(0).len(), 1);
    }

    #[test]
    fn delete_card_cancel_n() {
        let (_dir, mut app) = test_app();
        app.handle_key(make_key(KeyCode::Char('d')));
        assert_eq!(app.mode, Mode::Confirm);

        app.handle_key(make_key(KeyCode::Char('n')));
        assert_eq!(app.mode, Mode::Normal);
        assert_eq!(app.column_cards(0).len(), 2); // unchanged
    }

    #[test]
    fn help_mode_toggle() {
        let (_dir, mut app) = test_app();
        app.handle_key(make_key(KeyCode::Char('?')));
        assert_eq!(app.mode, Mode::Help);
        app.handle_key(make_key(KeyCode::Esc));
        assert_eq!(app.mode, Mode::Normal);
    }

    #[test]
    fn refresh_reloads_board() {
        let (_dir, mut app) = test_app();
        app.handle_key(make_key(KeyCode::Char('r')));
        assert!(app.message.as_ref().unwrap().contains("reloaded"));
    }

    #[test]
    fn switching_columns_clamps_row() {
        let (_dir, mut app) = test_app();
        // In todo col (2 items), select row 1
        app.handle_key(make_key(KeyCode::Char('j')));
        assert_eq!(app.selected_row, 1);

        // Move to doing (1 item) — row should clamp to 0
        app.handle_key(make_key(KeyCode::Char('l')));
        assert_eq!(app.selected_col, 1);
        assert_eq!(app.selected_row, 0);
    }

    #[test]
    fn empty_insert_does_nothing() {
        let (_dir, mut app) = test_app();
        app.handle_key(make_key(KeyCode::Char('a')));
        app.handle_key(make_key(KeyCode::Enter)); // empty title
        assert_eq!(app.column_cards(0).len(), 2); // unchanged
    }

    #[test]
    fn arrow_keys_work() {
        let (_dir, mut app) = test_app();
        app.handle_key(make_key(KeyCode::Down));
        assert_eq!(app.selected_row, 1);
        app.handle_key(make_key(KeyCode::Up));
        assert_eq!(app.selected_row, 0);
        app.handle_key(make_key(KeyCode::Right));
        assert_eq!(app.selected_col, 1);
        app.handle_key(make_key(KeyCode::Left));
        assert_eq!(app.selected_col, 0);
    }

    // --- Board picker tests ---

    fn test_app_with_boards() -> (TempDir, App) {
        let dir = TempDir::new().unwrap();
        let store = Store::new(dir.path());
        store.init().unwrap();

        // Create additional boards
        store
            .create_board(
                "sprint-1",
                vec![
                    crate::model::Column {
                        name: "todo".into(),
                        wip_limit: None,
                    },
                    crate::model::Column {
                        name: "doing".into(),
                        wip_limit: None,
                    },
                    crate::model::Column {
                        name: "done".into(),
                        wip_limit: None,
                    },
                ],
            )
            .unwrap();
        store
            .create_board(
                "backlog",
                vec![crate::model::Column {
                    name: "ideas".into(),
                    wip_limit: None,
                }],
            )
            .unwrap();

        let app = App::new(dir.path()).unwrap();
        (dir, app)
    }

    #[test]
    fn board_picker_opens_with_b() {
        let (_dir, mut app) = test_app_with_boards();
        app.handle_key(make_key(KeyCode::Char('b')));
        assert_eq!(app.mode, Mode::BoardPicker);
        assert_eq!(app.board_list.len(), 3); // backlog, default, sprint-1 (sorted)
        // Current board is "default", should be pre-selected
        let current_name = &app.board_list[app.board_selected];
        assert_eq!(current_name, "default");
    }

    #[test]
    fn board_picker_navigate_jk() {
        let (_dir, mut app) = test_app_with_boards();
        app.handle_key(make_key(KeyCode::Char('b')));
        let initial = app.board_selected;

        // Move down
        app.handle_key(make_key(KeyCode::Char('j')));
        assert_eq!(app.board_selected, initial + 1);

        // Move back up
        app.handle_key(make_key(KeyCode::Char('k')));
        assert_eq!(app.board_selected, initial);
    }

    #[test]
    fn board_picker_navigate_stays_in_bounds() {
        let (_dir, mut app) = test_app_with_boards();
        app.handle_key(make_key(KeyCode::Char('b')));

        // Move to top then try to go further up
        app.board_selected = 0;
        app.handle_key(make_key(KeyCode::Char('k')));
        assert_eq!(app.board_selected, 0);

        // Move to bottom then try to go further down
        app.board_selected = app.board_list.len() - 1;
        let last = app.board_selected;
        app.handle_key(make_key(KeyCode::Char('j')));
        assert_eq!(app.board_selected, last);
    }

    #[test]
    fn board_picker_cancel_esc() {
        let (_dir, mut app) = test_app_with_boards();
        app.handle_key(make_key(KeyCode::Char('b')));
        assert_eq!(app.mode, Mode::BoardPicker);

        app.handle_key(make_key(KeyCode::Esc));
        assert_eq!(app.mode, Mode::Normal);
        assert_eq!(app.board.name, "default"); // unchanged
    }

    #[test]
    fn board_picker_switch_board() {
        let (_dir, mut app) = test_app_with_boards();
        app.handle_key(make_key(KeyCode::Char('b')));

        // boards are sorted: backlog, default, sprint-1
        // current is "default" (index 1), navigate to "sprint-1" (index 2)
        let sprint_idx = app.board_list.iter().position(|b| b == "sprint-1").unwrap();
        app.board_selected = sprint_idx;

        app.handle_key(make_key(KeyCode::Enter));
        assert_eq!(app.mode, Mode::Normal);
        assert_eq!(app.board.name, "sprint-1");
        assert_eq!(app.selected_col, 0);
        assert_eq!(app.selected_row, 0);
        assert!(app.message.as_ref().unwrap().contains("sprint-1"));

        // Verify config was persisted
        let config = app.store.load_config().unwrap();
        assert_eq!(config.default_board, "sprint-1");
    }

    #[test]
    fn board_picker_select_same_board_noop() {
        let (_dir, mut app) = test_app_with_boards();
        app.handle_key(make_key(KeyCode::Char('b')));

        // "default" is already selected, press Enter
        let default_idx = app.board_list.iter().position(|b| b == "default").unwrap();
        app.board_selected = default_idx;

        app.handle_key(make_key(KeyCode::Enter));
        assert_eq!(app.mode, Mode::Normal);
        assert_eq!(app.board.name, "default");
    }

    #[test]
    fn board_picker_arrow_keys() {
        let (_dir, mut app) = test_app_with_boards();
        app.handle_key(make_key(KeyCode::Char('b')));
        let initial = app.board_selected;

        app.handle_key(make_key(KeyCode::Down));
        assert_eq!(app.board_selected, initial + 1);

        app.handle_key(make_key(KeyCode::Up));
        assert_eq!(app.board_selected, initial);
    }

    #[test]
    fn board_picker_resets_search_on_switch() {
        let (_dir, mut app) = test_app_with_boards();

        // Activate search first
        app.handle_key(make_key(KeyCode::Char('/')));
        app.handle_key(make_key(KeyCode::Char('x')));
        app.handle_key(make_key(KeyCode::Enter));
        assert!(app.search_active);

        // Open board picker and switch
        app.handle_key(make_key(KeyCode::Char('b')));
        let sprint_idx = app.board_list.iter().position(|b| b == "sprint-1").unwrap();
        app.board_selected = sprint_idx;
        app.handle_key(make_key(KeyCode::Enter));

        assert!(!app.search_active);
        assert!(app.search_buf.is_empty());
    }
}
