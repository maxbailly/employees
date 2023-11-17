use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use signal_hook::consts::TERM_SIGNALS;
use signal_hook::flag;

/* ---------- */

/// Enables the runtime to be gracefully shutdown.
///
/// If for some reasons, the runtime isn't shutdown after the first signal,
/// users can send another signal to kill ungracefully the runtime.
#[inline]
pub(crate) fn enable_graceful_shutdown(shutdown: &Shutdown) {
    for sig in TERM_SIGNALS {
        // TODO: Print du log, retourner une erreur ou juste ne rien faire ?
        let _ = flag::register_conditional_shutdown(*sig, 1, shutdown.as_ref().clone());
        let _ = flag::register(*sig, shutdown.as_ref().clone());
    }
}

/* ---------- */

/// Describes the running status of a [`Runtime`].
///
/// Primary used when the gracefull shutdown is enabled,
/// some actors may use it to control the runtime in which they are launched.
///
/// [`Runtime`]: crate::Runtime
#[derive(Debug, Default)]
pub struct Shutdown(Arc<AtomicBool>);

impl Shutdown {
    #[inline]
    pub(crate) fn new() -> Self {
        Self::default()
    }

    #[inline]
    pub(crate) fn stop(&self) {
        self.0.store(true, Ordering::SeqCst)
    }

    /// Returns whether or not the [`Runtime`] is running.
    ///
    /// [`Runtime`]: crate::Runtime
    #[inline]
    pub fn is_running(&self) -> bool {
        !self.0.load(Ordering::SeqCst)
    }
}

impl AsRef<Arc<AtomicBool>> for Shutdown {
    #[inline]
    fn as_ref(&self) -> &Arc<AtomicBool> {
        &self.0
    }
}

impl Clone for Shutdown {
    #[inline]
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
