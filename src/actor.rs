use std::ops::DerefMut;

use crate::settings::Settings;
use crate::utils::Shutdown;
use crate::Error;

/* ---------- */

/// A trait to implement on types that behave as actors.
pub trait Worker: Send {
    /// First method to be called when the [`Actor`]'s thread is spawned.
    ///
    /// By default, this does nothing. One can override this method to
    /// add some behaviour when the thread is spawned.
    #[inline]
    fn on_start(&mut self) {}

    /// Called in a infinite loop, until either [`ControlFlow::Break`] is returned
    /// or the runtime in which the actor runs is shutdown.
    ///
    /// By default, this method just returns [`ControlFlow::Break`].
    #[inline]
    fn on_update(&mut self) -> ControlFlow {
        ControlFlow::Break
    }

    /// Main loop of an actor.
    ///
    /// Called by the runtime when using calling one of the [`Runtime::launch`] functions.
    ///
    /// By default, this first calls [`Actor::on_start`] then [`Actor::on_update`] in a loop that spins until [`shutdown.is_running()`]
    /// returns `false`.
    ///
    /// One can override this to change the behaviour of an actor's main loop or when specific needs aren't provided by the default
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

impl<T: Worker> Worker for Box<T> {
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

/// A trait to build an [`Actor`] from values computed before launching said actor.
pub trait Context {
    /// The type of the [`Actor`] related to this this context.
    type Target: Worker;

    /// Consumes `self` to build the targeted [`Actor`] from the context.
    fn into_actor(self) -> Result<Self::Target, Error>;

    /// Returns some [`Settings`] used to configure runtime threads.
    ///
    /// When not implemented, it returns default [`Settings`].
    #[inline]
    fn settings(&self) -> Settings {
        Settings::default()
    }
    /// Returns some cpu IDs to pin [`Actor`] threads to.
    ///
    /// When not implemented, it returns `None`.
    #[inline]
    fn core_pinning(&self) -> Option<Vec<usize>> {
        None
    }
}

/* ---------- */

pub trait RespawnableContext<'a> {
    fn boxed_actor(&self) -> Result<Box<dyn Worker + 'a>, Error>;

    /// Returns some [`Settings`] used to configure runtime threads.
    ///
    /// When not implemented, it returns default [`Settings`].
    fn settings(&self) -> Settings {
        Settings::default()
    }
    /// Returns some cpu IDs to pin [`Actor`] threads to.
    ///
    /// When not implemented, it returns `None`.
    fn core_pinning(&self) -> Option<Vec<usize>> {
        None
    }
}

/* ---------- */

/// Defines variants to allow an [`Actor`] to shut itself down prematurely.
#[derive(Debug, PartialEq)]
pub enum ControlFlow {
    /// Tells the runtime to continue the actor's loop.
    Continue,
    /// Tells the runtime to break the actor's loop.
    Break,
}
