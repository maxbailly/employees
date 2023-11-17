use std::fmt::{Debug, Formatter, Result};
use std::thread::Builder;

/* ---------- */

/// Used to configure the properties of a new actor's thread.
pub struct Settings(Builder);

impl Settings {
    /// Returns the base [`Settings`] with default parameters.
    #[inline]
    pub fn new() -> Self {
        Self(Builder::new())
    }

    /// Sets the thread's name.
    ///
    /// The name must not contains null bytes (`\0`).
    #[inline]
    pub fn name<T: ToString>(self, name: T) -> Self {
        Self(self.0.name(name.to_string()))
    }

    /// Sets the thread's stack size.
    ///
    /// The actual stack size may be greater than this value if the platform specifies a minimal stack size.
    #[inline]
    pub fn stack_size(self, size: usize) -> Self {
        Self(self.0.stack_size(size))
    }

    /// Returns the inner [`std::thread::Builder`].
    #[inline]
    pub(crate) fn into_inner(self) -> Builder {
        self.0
    }
}

impl Default for Settings {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl Debug for Settings {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        write!(f, "{:?}", self.0)
    }
}
