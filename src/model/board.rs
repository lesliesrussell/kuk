use serde::{Deserialize, Serialize};

use super::Card;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Column {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wip_limit: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Board {
    pub name: String,
    pub columns: Vec<Column>,
    pub cards: Vec<Card>,
}

impl Board {
    pub fn default_board() -> Self {
        Self {
            name: "default".into(),
            columns: vec![
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
            ],
            cards: Vec::new(),
        }
    }

    pub fn has_column(&self, name: &str) -> bool {
        self.columns.iter().any(|c| c.name == name)
    }

    pub fn next_order(&self, column: &str) -> u32 {
        self.cards
            .iter()
            .filter(|c| c.column == column && !c.archived)
            .map(|c| c.order)
            .max()
            .map(|m| m + 1)
            .unwrap_or(0)
    }

    pub fn find_card(&self, id: &str) -> Option<&Card> {
        self.cards.iter().find(|c| c.id == id)
    }

    pub fn find_card_mut(&mut self, id: &str) -> Option<&mut Card> {
        self.cards.iter_mut().find(|c| c.id == id)
    }

    /// Find a card by 1-based display number within a column.
    /// Cards are ordered by their `order` field ascending, non-archived only.
    pub fn find_card_by_number(&self, number: usize) -> Option<&Card> {
        let mut active: Vec<&Card> = self.cards.iter().filter(|c| !c.archived).collect();
        active.sort_by_key(|c| c.order);
        active.get(number.wrapping_sub(1)).copied()
    }

    /// Resolve an ID string: either a ULID or a 1-based number.
    pub fn resolve_card_id(&self, id_or_num: &str) -> Option<String> {
        if let Ok(num) = id_or_num.parse::<usize>() {
            self.find_card_by_number(num).map(|c| c.id.clone())
        } else {
            self.find_card(id_or_num).map(|c| c.id.clone())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_board_has_three_columns() {
        let board = Board::default_board();
        assert_eq!(board.name, "default");
        assert_eq!(board.columns.len(), 3);
        assert_eq!(board.columns[0].name, "todo");
        assert_eq!(board.columns[1].name, "doing");
        assert_eq!(board.columns[2].name, "done");
    }

    #[test]
    fn has_column() {
        let board = Board::default_board();
        assert!(board.has_column("todo"));
        assert!(board.has_column("doing"));
        assert!(board.has_column("done"));
        assert!(!board.has_column("blocked"));
    }

    #[test]
    fn next_order_empty_column() {
        let board = Board::default_board();
        assert_eq!(board.next_order("todo"), 0);
    }

    #[test]
    fn next_order_with_cards() {
        let mut board = Board::default_board();
        let mut c1 = Card::new("A", "todo");
        c1.order = 0;
        let mut c2 = Card::new("B", "todo");
        c2.order = 1;
        board.cards.push(c1);
        board.cards.push(c2);
        assert_eq!(board.next_order("todo"), 2);
        assert_eq!(board.next_order("doing"), 0);
    }

    #[test]
    fn next_order_ignores_archived() {
        let mut board = Board::default_board();
        let mut c1 = Card::new("A", "todo");
        c1.order = 5;
        c1.archived = true;
        board.cards.push(c1);
        assert_eq!(board.next_order("todo"), 0);
    }

    #[test]
    fn find_card_by_id() {
        let mut board = Board::default_board();
        let card = Card::new("Find me", "todo");
        let id = card.id.clone();
        board.cards.push(card);
        assert!(board.find_card(&id).is_some());
        assert!(board.find_card("nonexistent").is_none());
    }

    #[test]
    fn find_card_by_number() {
        let mut board = Board::default_board();
        let mut c1 = Card::new("First", "todo");
        c1.order = 0;
        let mut c2 = Card::new("Second", "doing");
        c2.order = 1;
        board.cards.push(c1);
        board.cards.push(c2);
        let found = board.find_card_by_number(1).unwrap();
        assert_eq!(found.title, "First");
        let found = board.find_card_by_number(2).unwrap();
        assert_eq!(found.title, "Second");
        assert!(board.find_card_by_number(0).is_none());
        assert!(board.find_card_by_number(99).is_none());
    }

    #[test]
    fn resolve_card_id_by_number() {
        let mut board = Board::default_board();
        let card = Card::new("Resolve me", "todo");
        let id = card.id.clone();
        board.cards.push(card);
        assert_eq!(board.resolve_card_id("1"), Some(id.clone()));
        assert_eq!(board.resolve_card_id(&id), Some(id));
        assert!(board.resolve_card_id("99").is_none());
    }

    #[test]
    fn board_roundtrip_json() {
        let mut board = Board::default_board();
        board.cards.push(Card::new("Task 1", "todo"));
        let json = serde_json::to_string_pretty(&board).unwrap();
        let deserialized: Board = serde_json::from_str(&json).unwrap();
        assert_eq!(board, deserialized);
    }
}
