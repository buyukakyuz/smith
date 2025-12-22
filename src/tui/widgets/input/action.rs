#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputAction {
    Continue,
    Submit(String),
    HistoryPrev,
    HistoryNext,
    Clear,
}
