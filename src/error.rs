#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("invalid context: {0}")]
    InvalidContext(String),

    #[error(transparent)]
    ThreadStart(#[from] std::io::Error),
}
