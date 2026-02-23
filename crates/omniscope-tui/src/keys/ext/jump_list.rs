// use omniscope_core::BookSummaryView;

/// Represents a position in the jump list.
/// For now, we store the book ID (or index) and maybe panel/library context.
/// Storing index is fragile if list changes, storing ID is better but requires lookup.
/// Vim stores (file, line, col). We can store (BookID).
/// But for fast jumping in current view, maybe just index if list hasn't changed?
/// Let's stick to simple index for Phase 1 if that's what we have, or better:
/// We need to restore the view state.
/// Let's store `(usize)` for now, ensuring we check bounds.
/// Actually, `JumpList` in the plan says: `(library, folder, book_id)`.
/// But since we don't have full `library`/`folder` structure exposed easily in `App` yet (it's flat list),
/// we'll start with just `usize` (index) and maybe `String` (book_id).
#[derive(Debug, Clone, PartialEq)]
pub struct JumpLoc {
    pub index: usize,
    pub book_id: String, // fallback if index is out of sync
}

#[derive(Debug, Clone, Default)]
pub struct JumpList {
    pub jumps: Vec<JumpLoc>,
    /// Current position in the jump list.
    /// Points to the entry representing the *current* position if we traversed.
    /// If we are at the tip (newest), it might be `len`.
    pub current: usize,
}

impl JumpList {
    pub fn new() -> Self {
        Self {
            jumps: Vec::new(),
            current: 0,
        }
    }

    /// Add a new jump location. clears future if specific logic (like non-jump motion) - wait,
    /// usually jumping adds to top, and if we were back in history, it bifurcates or clears forward?
    /// Standard vim: `Ctrl+O` goes back. New jump adds to end.
    /// If we are in middle of list and make a NEW jump, we usually truncate the "future" or branch.
    /// Vim implementation: "When you make a jump... the current position is added formatted into the jump list... and the new position...".
    /// If you navigate with simple motions, nothing happens.
    /// If you navigate with jump motion (gg, G, /), push current pos.
    pub fn push(&mut self, index: usize, book_id: String) {
        // If we are not at the end, truncate future?
        if self.current < self.jumps.len() {
            self.jumps.truncate(self.current + 1);
        }

        // Avoid duplicates at top
        if let Some(last) = self.jumps.last() {
            if last.index == index && last.book_id == book_id {
                return;
            }
        }

        self.jumps.push(JumpLoc { index, book_id });
        if self.jumps.len() > 100 {
            self.jumps.remove(0);
        }
        self.current = self.jumps.len();
    }

    pub fn back(&mut self) -> Option<&JumpLoc> {
        if self.current > 0 {
            self.current -= 1;
            self.jumps.get(self.current)
        } else {
            None
        }
    }

    pub fn forward(&mut self) -> Option<&JumpLoc> {
        if self.current + 1 < self.jumps.len() {
            self.current += 1;
            self.jumps.get(self.current)
        } else {
            None
        }
    }
}
