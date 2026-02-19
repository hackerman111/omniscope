/// Standard Vim operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operator {
    Delete,
    Yank,
    Change,
    Move,
    Put,
    AddTag,
    RemoveTag,
    Normalize,
    Filter, // e.g. =
}
