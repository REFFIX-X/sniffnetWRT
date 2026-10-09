//! Sort direction (minimal, decoupled from the GUI sort UI).

/// How to order a data series.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SortType {
    /// Smallest first.
    Ascending,
    /// Largest first.
    Descending,
    /// Most-recently-seen first.
    #[default]
    Neutral,
}
