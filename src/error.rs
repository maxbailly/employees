//! The various errors implementations.

use std::error::Error as StdError;
use std::fmt::{Debug, Display, Formatter, Result};

/* ---------- */

/// Kinds of error that might happen.
pub enum Error {
    /// Something went wrong when building a worker from a context.
    Context(Box<dyn Display + Send + Sync>),
    /// Something went wrong when spawning a thread.
    Thread(std::io::Error),
    /// Other kinds of error.
    Other(Box<dyn StdError + Send + Sync>),
}

impl Error {
    /// Returns a new [`Error::Context`] from `reason`.
    pub fn context<D>(reason: D) -> Self
    where
        D: Display + Send + Sync + 'static,
    {
        Self::Context(Box::new(reason))
    }

    /// Returns a new [`Error::Thread`].
    pub fn thread(err: std::io::Error) -> Self {
        Self::Thread(err)
    }

    /// Returns a new [`Error::Other`] from some error.
    pub fn other<E>(err: E) -> Self
    where
        E: StdError + Send + Sync + 'static,
    {
        Self::Other(Box::new(err))
    }
}

impl Debug for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        match self {
            Error::Context(inner) => f.debug_tuple("Context").field(&format!("{inner}")).finish(),
            Error::Thread(inner) => f.debug_tuple("Thread").field(inner).finish(),
            Error::Other(inner) => f.debug_tuple("Other").field(inner).finish(),
        }
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        match self {
            Self::Context(reason) => write!(f, "Invalid context {reason}"),
            Self::Thread(err) => write!(f, "Failed to run worker thread: {err}"),
            Self::Other(err) => write!(f, "Failed launch worker: {err}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::Context(..) => None,
            Self::Thread(err) => Some(err),
            Self::Other(err) => Some(err.as_ref()),
        }
    }
}
