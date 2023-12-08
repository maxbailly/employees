use std::ops::DerefMut;

use crate::settings::Settings;
use crate::utils::Shutdown;
use crate::Error;

/* ---------- */

/// A worker represents a thread that runs for the lifetime of the program (or at least for a very long time).
///
/// Types that implement the [`Worker`] trait are called `workers`. Workers are independent of anything to do their job,
/// even of the runtime where they were launched, the data to work on being provided by the type itself.
///
/// Workers are defined by one main method, [`Worker::run`], which run the actual infinite loop. This method
/// has a default implementation that first calls the [`Worker::on_start`] method once at the beginning, then calls
/// [`Worker::on_update`] in the loop until it returns [`ControlFlow::Break`] or the runtime is stopped.
///
/// The last two methods exist as a convenience for simple workers that do not need specific instructions or
/// setup to be done before running their infinite loop. Default implementations are provided:
/// * [`Worker::on_start`] does nothing and returns immediately.
/// * [`Worker::on_update`] does nothing and returns [`ControlFlow::Break`] immediately.
///
/// # Examples
///
/// A dead simple worker that prints "Hello, World!" each 100ms for one second:
///
/// ```
/// # use employees::{Runtime, Worker, ControlFlow};
/// # use std::time::Duration;
/// struct WorkerThatPrints;
/// impl Worker for WorkerThatPrints {
///     fn on_update(&mut self) -> ControlFlow {
///         println!("Hello, World!");
///         std::thread::sleep(Duration::from_millis(100));
///         ControlFlow::Continue
///     }
/// }
///
/// let mut runtime = Runtime::new();
///
/// runtime.launch(WorkerThatPrints);
/// std::thread::sleep(Duration::from_secs(1));
/// runtime.stop();
/// ```
///
/// A worker that counts the number of time its update method has beed called and print that amount once dropped:
///
/// ```
/// # use employees::*;
/// # use std::time::Duration;
/// #[derive(Debug, Default)]
/// struct Counter {
///     count: usize
/// }
///
/// impl Worker for Counter {
///     fn on_update(&mut self) -> ControlFlow {
///         self.count += 1;
///
///         // Some heavy work...
/// #       std::thread::sleep(Duration::from_millis(50));
///
///         ControlFlow::Continue
///     }
/// }
///
/// impl Drop for Counter {
///     fn drop(&mut self) {
///         println!("num updates: {}", self.count);
///     }
/// }
///
/// let mut runtime = Runtime::new();
/// runtime.launch(Counter::default());
/// # std::thread::sleep(Duration::from_secs(1));
/// # runtime.stop();
/// ```
pub trait Worker: Send {
    /// Convenient method to print or set stuff up before entering the worker infinite loop.
    ///
    /// The first method to be called by the [`Worker::run`] default implementation.
    /// By default, this does nothing.
    #[inline]
    fn on_start(&mut self) {}

    /// Convenient method to do work on iteration of the worker loop.
    ///
    /// Called in a infinite loop by the [`Worker::run`] default implementation, until either
    /// [`ControlFlow::Break`] is returned or the runtime in which the worker runs is shutdown.
    /// By default, this method just returns [`ControlFlow::Break`].
    #[inline]
    fn on_update(&mut self) -> ControlFlow {
        ControlFlow::Break
    }

    /// Main worker loop spawned in a new thread by the runtime when using calling one of the [`Runtime::launch`] functions.
    ///
    /// By default, this first calls [`Worker::on_start`] then [`Worker::on_update`] in a loop that spins until [`shutdown.is_running()`]
    /// returns `false`.
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

/// Allow building a worker before actually launching it with the [`Runtime::launch_from_context`] function.
///
/// This trait exists as way to build and launch workers in a builder pattern way. For example, users can
/// can launch workers configured via a configuration file or command-line arguments.
///
/// [`Runtime::launch_from_context`]: crate::Runtime::launch_from_context
///
/// # Examples
///
/// ```
/// # use employees::*;
/// // Let's define a counter...
/// struct Counter {
///     counter: usize,
///     max: usize,
/// }
///
/// impl Counter {
///     fn new(max: usize) -> Self {
///         Self {
///             counter: 0,
///             max,
///         }
///     }
/// }
///
/// impl Worker for Counter {
///     fn on_update(&mut self) -> ControlFlow {
///         self.counter += 1;
///         if self.counter > self.max {
///             return ControlFlow::Break;
///         }
///
///         ControlFlow::Continue
///     }
/// }
///
/// // ... and the context to build it from.
/// struct CounterContext {
///     max: usize
/// }
///
/// impl CounterContext {
///     fn new(max: usize) -> Self {
///         Self { max }
///     }
/// }
///
/// impl Context for CounterContext {
///     type Target = Counter;
///
///     fn into_worker(self) -> Result<Self::Target, Error> {
///         Ok(Counter::new(self.max))
///     }
/// }
///
/// let mut runtime = Runtime::new();
/// let context = CounterContext::new(10);
///
/// runtime.launch_from_context(context);
/// ```
pub trait Context {
    /// The type of [`Worker`] built from this context.
    type Target: Worker;

    /// Consumes `self` to build the targeted [`Worker`] from the context.
    fn into_worker(self) -> Result<Self::Target, Error>;

    /// Returns some [`Settings`] used to configure runtime threads.
    ///
    /// By default, it returns default thread settings.
    #[inline]
    fn settings(&self) -> Settings {
        Settings::default()
    }

    /// Returns some cpu IDs to pin [`Worker`] threads to.
    ///
    /// By default, it returns `None`.
    #[inline]
    fn core_pinning(&self) -> Option<Vec<usize>> {
        None
    }
}

/* ---------- */

/// This trait is very similar to the [`Context`] but is needed for workers to be respawned.
///
/// Respawnable workers are launch with the [`Runtime::launch_respawnable`] function and
/// are respawned if they panicked with the [`Runtime::health_check`] one. See their
/// respective documentation for more details.
///
/// [`Runtime::launch_respawnable`]: crate::Runtime::launch_respawnable
/// [`Runtime::health_check`]: crate::Runtime::health_check
///
/// # Examples
///
/// ```
/// # use employees::*;
/// # use std::time::Duration;
/// // A worker that panic some time after being spawned...
/// struct PanickingWorker;
/// impl Worker for PanickingWorker {
///     fn on_update(&mut self) -> ControlFlow {
///         std::thread::sleep(Duration::from_secs(1));
///         panic!("panicking!")
///     }
/// }
///
/// // ... and its context.
/// struct WorkerContext;
/// impl RespawnableContext<'_> for WorkerContext {
///     fn boxed_worker(&self) -> Result<Box<dyn Worker>, Error> {
///         Ok(Box::new(PanickingWorker))
///     }
/// }
///
/// let mut runtime = Runtime::new();
/// runtime.launch_respawnable(WorkerContext);
///
/// std::thread::sleep(Duration::from_secs(1));
/// runtime.health_check();
/// # runtime.stop();
/// ```
pub trait RespawnableContext<'a> {
    /// Similar to [`Context::into_worker`] but this function doesn't consume `self`
    /// and returns a boxed dyn [`Worker`].
    fn boxed_worker(&self) -> Result<Box<dyn Worker + 'a>, Error>;

    /// Works exactly as [`Context::settings`], see its documentation for more details.
    #[inline]
    fn settings(&self) -> Settings {
        Settings::default()
    }

    /// Works exactly as [`Context::core_pinning`], see its documentation for more details.
    #[inline]
    fn core_pinning(&self) -> Option<Vec<usize>> {
        None
    }
}

/* ---------- */

/// Defines the control flow of [`Workers`].
///
/// [`Workers`]: crate::Worker
///
/// # Examples
///
/// A worker that counts to 10 and stops.
///
/// ```
/// # use employees::*;
/// #[derive(Debug, Default)]
/// struct Counter {
///     count: usize
/// }
///
/// impl Worker for Counter {
///     fn on_update(&mut self) -> ControlFlow {
///         self.count += 1;
///         println!("counter: {}", self.count);
///
///
///         // We're done counting, let's leave the loop.
///         if self.count > 10 {
///             return ControlFlow::Break;
///         }
///
///         // The loop continues.
///         ControlFlow::Continue
///     }
/// }
///
/// let mut runtime = Runtime::new();
/// runtime.launch(Counter::default());
/// ```
#[derive(Debug, PartialEq)]
pub enum ControlFlow {
    /// Tells the runtime to continue the main worker loop.
    Continue,
    /// Tells the runtime to break the main worker loop.
    Break,
}
