pub mod bundle;
pub mod serialized;
pub mod typetree;

#[derive(Debug, thiserror::Error)]
pub enum UnityError {
    #[error("i/o: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid file: {0}")]
    Format(String),
    #[error("unsupported: {0}")]
    Unsupported(String),
    /// The embedded schema no longer matches the game's data layout,
    /// usually after a game update.
    #[error("layout mismatch: {0}")]
    LayoutMismatch(String),
}
