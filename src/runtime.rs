use std::marker::PhantomData;
use std::thread::JoinHandle;

use crate::settings::Settings;
use crate::utils::{Nested, Root, Shutdown, Type};
use crate::worker::{Context, RespawnableContext, Worker};
use crate::Error;

/* ---------- */

/// A runtime that manages [`Workers`] threads.
///
/// When dropped, a runtime waits for all the threads to complete.
///
/// [`Workers`]: crate::Worker
#[derive(Default)]
pub struct Runtime<T: Type> {
    shutdown: Shutdown,
    threads: Vec<JoinHandle<()>>,
    respawnable: Vec<RespawnableHandle>,

    _type: PhantomData<T>,
}

impl Runtime<Root> {
    /// Returns a new runtime.
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    /// Enables this runtime to be gracefully shutdown with a `Ctrl+C` signal.
    ///
    /// If the gracefull shutdown doesn't have any effects, users can still
    /// send a second `Ctrl+C` signal to forcefully kill the runtime.
    #[inline]
    pub fn enable_graceful_shutdown(&self) {
        crate::utils::enable_graceful_shutdown(&self.shutdown)
    }

    /// Forces workers to leave their main loop.
    ///
    /// This function doesn't wait for thread completion.
    #[inline]
    pub fn stop(&self) {
        self.shutdown.stop()
    }
}

impl Runtime<Nested> {
    /// Returns a new runtime whose stopping condition is controlled by the "parent" runtime
    /// from which `shutdown` is originates.
    ///
    /// This allows users to spawn runtimes in workers without caring about the shutdown.
    /// Thus, the [`enable_graceful_shutdown`] and [`stop`] functions can't be called on a nested runtime.
    ///
    /// [`enable_graceful_shutdown`]: Runtime::enable_graceful_shutdown
    /// [`stop`]: Runtime::stop
    #[inline]
    pub fn nested(shutdown: Shutdown) -> Self {
        Self::from(shutdown)
    }
}

impl<T: Type> Runtime<T> {
    /// Runs a [`Worker`] in a new thread.
    ///
    /// # Examples
    ///
    /// ```
    /// # use employees::*;
    /// struct Employee;
    /// // -- skipping the Worker implementation for Employee...
    /// # impl Worker for Employee {}
    ///
    /// let mut runtime = Runtime::new();
    ///
    /// // Run a Employee thread.
    /// runtime.launch(Employee).unwrap();
    /// ```
    #[inline]
    pub fn launch<W: Worker + 'static>(&mut self, worker: W) -> Result<(), Error> {
        self.inner_spawn_thread(worker, Settings::default(), None::<Vec<_>>)
    }

    /// Runs a [`Worker`] in a new thread.
    ///
    /// The new thread will be configured with `settings`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use employees::*;
    /// struct Employee;
    /// // -- skipping the Worker implementation for Employee...
    /// # impl Worker for Employee {}
    ///
    /// let mut runtime = Runtime::new();
    /// let settings = Settings::new().name("alice");
    ///
    /// // Run a Employee thread named "alice".
    /// runtime.launch_with_settings(Employee, settings).unwrap();
    /// ```
    #[inline]
    pub fn launch_with_settings<W: Worker + 'static>(
        &mut self,
        worker: W,
        settings: Settings,
    ) -> Result<(), Error> {
        self.inner_spawn_thread(worker, settings, None::<Vec<_>>)
    }

    /// Runs a [`Worker`] in a new thread.
    ///
    /// The new thread will have its affinity set to the `cores`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use employees::*;
    /// struct Employee;
    /// // -- skipping the Worker implementation for Employee...
    /// # impl Worker for Employee {}
    ///
    /// let mut runtime = Runtime::new();
    ///
    /// // Run a Employee thread bound to the CPU #1.
    /// runtime.launch_pinned(Employee, [1]).unwrap();
    /// ```
    #[inline]
    pub fn launch_pinned<W, C>(&mut self, worker: W, cores: C) -> Result<(), Error>
    where
        W: Worker + 'static,
        C: AsRef<[usize]> + Send + 'static,
    {
        self.inner_spawn_thread(worker, Settings::default(), Some(cores))
    }

    /// Runs a [`Worker`] in a new thread.
    ///
    /// The new thread will be configured with `settings` and its affinity set to the `cores`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use employees::*;
    /// struct Employee;
    /// // -- skipping the Worker implementation for Employee...
    /// # impl Worker for Employee {}
    ///
    /// let mut runtime = Runtime::new();
    /// let settings = Settings::new().name("alice");
    ///
    /// // Run a Employee thread named "alice" bound to the CPU #1.
    /// runtime.launch_pinned_with_settings(Employee, [1], settings).unwrap();
    /// ```
    #[inline]
    pub fn launch_pinned_with_settings<W, C>(
        &mut self,
        worker: W,
        cores: C,
        settings: Settings,
    ) -> Result<(), Error>
    where
        W: Worker + 'static,
        C: AsRef<[usize]> + Send + 'static,
    {
        self.inner_spawn_thread(worker, settings, Some(cores))
    }

    /// Runs a [`Worker`] built from a [`Context`] in a new thread.
    ///
    /// The new thread will be configured using the values returned by the [`Context::settings`] function
    /// and its affinity set using the [`Context::core_pinning`] function.
    ///
    /// # Examples
    ///
    /// ```
    /// # use employees::*;
    /// struct Employee;
    /// // -- skipping the Worker implementation for Employee...
    /// # impl Worker for Employee {}
    ///
    /// struct EmployeeContext;
    /// impl Context for EmployeeContext {
    ///     // -- skipping the building of Employee...
    /// #   type Target = Employee;
    /// #   fn into_worker(self) -> Result<Self::Target, Error> {
    /// #       Ok(Employee)
    /// #   }
    ///
    ///     fn settings(&self) -> Settings {
    ///         // Setting the thread name.
    ///         Settings::default().name("employee")
    ///     }
    ///
    ///     fn core_pinning(&self) -> Option<Vec<usize>> {
    ///         // Setting the thread affinity on the CPU #1.
    ///         Some(vec![1])
    ///     }
    /// }
    ///
    /// let mut runtime = Runtime::new();
    /// runtime.launch_from_context(EmployeeContext).unwrap();
    /// ```
    #[inline]
    pub fn launch_from_context<W, C>(&mut self, ctx: C) -> Result<(), Error>
    where
        W: Worker + 'static,
        C: Context<Target = W>,
    {
        let settings = ctx.settings();
        let cores = ctx.core_pinning();
        let worker = ctx.into_worker()?;

        self.inner_spawn_thread(worker, settings, cores)
    }

    /// Runs a [`Worker`] built from a [`RespawnableContext`] that can be respawned if it panics.
    ///
    /// Similar to [`Runtime::launch_from_context`], see its documentation for more details.
    #[inline]
    pub fn launch_respawnable<C>(&mut self, ctx: C) -> Result<(), Error>
    where
        C: RespawnableContext<'static> + 'static,
    {
        let managed = RespawnableHandle::spawn_managed(ctx, &self.shutdown)?;

        self.respawnable.push(managed);
        Ok(())
    }

    /// Checks all respawnable [`Workers`], respawning the ones that panicked.
    ///
    /// Workers that finished but didn't panic are simply dropped.
    ///
    /// [`Workers`]: crate::Worker
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
    #[inline]
    pub fn health_check(&mut self) {
        self.respawnable.iter_mut().for_each(|managed| {
            // TODO: Do something with the errors
            let _ = managed.respawn_if_panicked(&self.shutdown);
        })
    }

    #[inline]
    fn inner_spawn_thread<W, C>(
        &mut self,
        worker: W,
        settings: Settings,
        cores: Option<C>,
    ) -> Result<(), Error>
    where
        W: Worker + 'static,
        C: AsRef<[usize]> + Send + 'static,
    {
        let thread = crate::utils::spawn_thread(worker, settings, cores, &self.shutdown)?;

        self.threads.push(thread);
        Ok(())
    }
}

impl Default for Runtime<Root> {
    #[inline]
    fn default() -> Self {
        Self {
            shutdown: Shutdown::new(),
            threads: Vec::new(),
            respawnable: Vec::new(),
            _type: PhantomData,
        }
    }
}

impl From<Shutdown> for Runtime<Nested> {
    #[inline]
    fn from(shutdown: Shutdown) -> Self {
        Self {
            shutdown,
            threads: Vec::new(),
            respawnable: Vec::new(),
            _type: PhantomData,
        }
    }
}

impl<T: Type> Drop for Runtime<T> {
    fn drop(&mut self) {
        for thread in self.threads.drain(..) {
            let _ = thread.join();
        }

        for thread in self.respawnable.drain(..) {
            thread.join();
        }
    }
}

/* ---------- */

struct RespawnableHandle {
    handle: Option<JoinHandle<()>>,
    context: Box<dyn RespawnableContext<'static>>,
}

impl RespawnableHandle {
    #[inline]
    fn spawn_managed(
        ctx: impl RespawnableContext<'static> + 'static,
        shutdown: &Shutdown,
    ) -> Result<Self, Error> {
        let cores = ctx.core_pinning();
        let settings = ctx.settings();
        let worker = ctx.boxed_worker()?;

        let thread = crate::utils::spawn_thread(worker, settings, cores, shutdown)?;

        Ok(Self {
            handle: Some(thread),
            context: Box::new(ctx),
        })
    }

    #[inline]
    fn is_finished(&self) -> bool {
        self.handle
            .as_ref()
            .map(|handle| handle.is_finished())
            .unwrap_or(true)
    }

    #[inline]
    fn join(mut self) {
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }

    fn respawn_if_panicked(&mut self, shutdown: &Shutdown) -> Result<(), Error> {
        if !self.is_finished() || self.handle.is_none() {
            return Ok(());
        }

        // SAFETY:
        // At this point, self.handle is always Some.
        let handle = unsafe { self.handle.take().unwrap_unchecked() };
        if handle.join().is_err() {
            let cores = self.context.core_pinning();
            let settings = self.context.settings();
            let worker = self.context.boxed_worker()?;

            let thread = crate::utils::spawn_thread(worker, settings, cores, shutdown)?;
            self.handle = Some(thread);
        }

        Ok(())
    }
}

/* ---------- */

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use rand::Rng;

    use super::*;
    use crate::test_utils::*;

    #[test]
    fn new_runtime() {
        let rt = Runtime::new();
        rt.stop()
    }

    #[test]
    fn start_stop() {
        let mut rt = Runtime::new();

        rt.launch(TestWorker)
            .expect("failed to launch the test actor");
        std::thread::sleep(Duration::from_millis(500));
        rt.stop();
    }

    #[test]
    fn drop() {
        let mut rt = Runtime::new();
        let now = Instant::now();
        let timeout = Duration::from_millis(500);

        rt.launch(TestTimedWorker::new(timeout))
            .expect("failed to launch the test actor");

        std::mem::drop(rt);
        assert!(now.elapsed() > timeout);
    }

    #[test]
    fn pinned_actor() {
        let mut rt = Runtime::new();
        let core_id = rand::thread_rng().gen_range(0..5);

        rt.launch_pinned(TestPinnedWorker::new(core_id), [core_id])
            .expect("failed to launch the test actor");
        std::thread::sleep(Duration::from_millis(1));
        rt.stop();
    }
}
