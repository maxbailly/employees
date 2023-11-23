use std::ops::DerefMut;

use crate::settings::Settings;
use crate::utils::Shutdown;
use crate::Error;

/* ---------- */

/// A trait to implement on types that behave as workers.
pub trait Worker: Send {
    /// First method to be called when the [`Worker`]'s thread is spawned.
    ///
    /// By default, this does nothing. One can override this method to
    /// add some behaviour when the thread is spawned.
    #[inline]
    fn on_start(&mut self) {}

    /// Called in a infinite loop, until either [`ControlFlow::Break`] is returned
    /// or the runtime in which the worker runs is shutdown.
    ///
    /// By default, this method just returns [`ControlFlow::Break`].
    #[inline]
    fn on_update(&mut self) -> ControlFlow {
        ControlFlow::Break
    }

    /// Main loop of an worker.
    ///
    /// Called by the runtime when using calling one of the [`Runtime::launch`] functions.
    ///
    /// By default, this first calls [`Worker::on_start`] then [`Worker::on_update`] in a loop that spins until [`shutdown.is_running()`]
    /// returns `false`.
    ///
    /// One can override this to change the behaviour of an worker's main loop or when specific needs aren't provided by the default
    /// implementation.
    ///
    /// [`Runtime::launch`]: crate::Runtime::launch
    /// [`shutdown.is_running()`]: crate::Shutdown::is_running
    #[inline]
    fn run(&mut self, shutdown: Shutdown) {
        self.on_start();

        while shutdown.is_running() {
            if let ControlFlow::Break = self.on_update() {
                break;
            }
        }
    }
}

impl<T: Worker + ?Sized> Worker for Box<T> {
    #[inline]
    fn on_start(&mut self) {
        self.deref_mut().on_start()
    }

    #[inline]
    fn on_update(&mut self) -> ControlFlow {
        self.deref_mut().on_update()
    }

    #[inline]
    fn run(&mut self, shutdown: Shutdown) {
        self.deref_mut().run(shutdown)
    }
}

/* ---------- */

/// A trait to build an [`Worker`] from values computed before launching said worker.
pub trait Context {
    /// The type of the [`Worker`] related to this this context.
    type Target: Worker;

    /// Consumes `self` to build the targeted [`Worker`] from the context.
    fn into_worker(self) -> Result<Self::Target, Error>;

    /// Returns some [`Settings`] used to configure runtime threads.
    ///
    /// When not implemented, it returns default [`Settings`].
    #[inline]
    fn settings(&self) -> Settings {
        Settings::default()
    }

    /// Returns some cpu IDs to pin [`Worker`] threads to.
    ///
    /// When not implemented, it returns `None`.
    #[inline]
    fn core_pinning(&self) -> Option<Vec<usize>> {
        None
    }
}

/* ---------- */

/// This trait is very similar to the [`Context`] but is needed for worker to be respawned.
pub trait RespawnableContext<'a> {
    /// Similar to [`Context::into_worker`] but this function doesn't consume `self`
    /// and returns a boxed dyn [`Worker`].
    fn boxed_worker(&self) -> Result<Box<dyn Worker + 'a>, Error>;

    /// Returns some [`Settings`] used to configure runtime threads.
    ///
    /// When not implemented, it returns default [`Settings`].
    fn settings(&self) -> Settings {
        Settings::default()
    }

    /// Returns some cpu IDs to pin [`Worker`] threads to.
    ///
    /// When not implemented, it returns `None`.
    fn core_pinning(&self) -> Option<Vec<usize>> {
        None
    }
}

/* ---------- */

/// Defines the control flow of [`Worker`]s.
#[derive(Debug, PartialEq)]
pub enum ControlFlow {
    /// Tells the runtime to continue the actor's loop.
    Continue,
    /// Tells the runtime to break the actor's loop.
    Break,
}
