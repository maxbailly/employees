//! Miscellaneous stuff and utils.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{JoinHandle, Scope, ScopedJoinHandle};

use signal_hook::consts::TERM_SIGNALS;
use signal_hook::flag;

use crate::settings::Settings;
use crate::{Error, Worker};

/* ---------- */

/// Enables the runtime to be gracefully shutdown.
///
/// If for some reasons, the runtime isn't shutdown after the first signal,
/// users can send another signal to kill ungracefully the runtime.
#[inline]
pub(crate) fn enable_graceful_shutdown(shutdown: &Shutdown) {
    for sig in TERM_SIGNALS {
        // TODO: Do we want to print some logs or return an error here?
        let _ = flag::register_conditional_shutdown(*sig, 1, shutdown.as_ref().clone());
        let _ = flag::register(*sig, shutdown.as_ref().clone());
    }
}

/* ---------- */

pub(crate) fn spawn_thread<W, C>(
    mut worker: W,
    settings: Settings,
    cores: Option<C>,
    shutdown: &Shutdown,
) -> Result<JoinHandle<()>, Error>
where
    W: Worker + 'static,
    C: AsRef<[usize]> + Send + 'static,
{
    let shutdown = shutdown.clone();

    settings
        .into_inner()
        .spawn(move || {
            if let Some(cores) = cores.as_ref() {
                let _ = affinity::set_thread_affinity(cores);
            }

            worker.run(shutdown);
        })
        .map_err(Error::thread)
}

/* ---------- */

pub(crate) fn spawn_scoped_thread<'scope, 'env, W, C>(
    scope: &'scope Scope<'scope, 'env>,
    mut worker: W,
    settings: Settings,
    cores: Option<C>,
    shutdown: &Shutdown,
) -> Result<ScopedJoinHandle<'scope, ()>, Error>
where
    W: Worker + 'env,
    C: AsRef<[usize]> + Send + 'env,
{
    let shutdown = shutdown.clone();

    settings
        .into_inner()
        .spawn_scoped(scope, move || {
            if let Some(cores) = cores.as_ref() {
                let _ = affinity::set_thread_affinity(cores);
            }

            worker.run(shutdown);
        })
        .map_err(Error::thread)
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

    /// Stops the [`Runtime`].
    ///
    /// [`Runtime`]: crate::Runtime
    #[inline]
    pub fn stop(&self) {
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

/* ---------- */

// To prevent users from implementing the Type trait.
mod private {
    pub trait Sealed {}
}

/// The type of a runtime.
///
/// [`Runtimes`] can have two different state that defines their ability to stop the execution of their workers.
/// Particularely, a [`Root`] runtime can stop its execution itself by calling the `enable_graceful_shutdown()` functions.
/// [`Nested`] runtimes, on the other hand, can't do this as their stop condition is inherited by the "parent" runtime.
///
/// [`Runtimes`]: crate::Runtime
pub trait Type: private::Sealed {
    /// Is the [`Runtime`] root or nested?
    ///
    /// [`Runtime`]: crate::Runtime
    const IS_ROOT: bool;
}

/// Marker type for a default runtime.
///
/// See the [`Type`] documentation for more informations.
#[derive(Debug)]
pub enum Root {}

impl Type for Root {
    const IS_ROOT: bool = true;
}

impl private::Sealed for Root {}

/// Marker type for a runtime spawned in and controlled by another runtime.
///
/// See the [`Type`] documentation for more informations.
#[derive(Debug)]
pub enum Nested {}

impl Type for Nested {
    const IS_ROOT: bool = false;
}

impl private::Sealed for Nested {}
