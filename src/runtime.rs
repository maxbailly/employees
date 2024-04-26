use std::thread::JoinHandle;
use std::time::Duration;

use crate::settings::Settings;
use crate::utils::Shutdown;
use crate::worker::{Context, RespawnableContext, Worker};
use crate::Error;

/* ---------- */

/// A runtime that manages [`Workers`] threads.
///
/// When dropped, a runtime stops and waits for all the workers to complete.
///
/// [`Workers`]: crate::Worker
pub struct Runtime {
    shutdown: Shutdown,
    threads: Vec<JoinHandle<()>>,
    respawnables: Vec<RespawnableHandle>,
    nested: bool,
}

impl Runtime {
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

    /// Returns a new runtime whose stopping condition is controlled by the "parent" runtime
    /// from which `shutdown` is originates.
    ///
    /// This allows users to spawn runtimes in workers without caring about the shutdown.
    #[inline]
    pub fn nested(shutdown: Shutdown) -> Self {
        Self::from(shutdown)
    }

    /// Runs a [`Worker`] in a new thread.
    ///
    /// # Errors
    ///
    /// On error, the corresponding error is returned and the runtime is stopped.
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
    /// # Errors
    ///
    /// On error, the corresponding error is returned and the runtime is stopped.
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
    /// # Errors
    ///
    /// On error, the corresponding error is returned and the runtime is stopped.
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
    /// # Errors
    ///
    /// On error, the corresponding error is returned and the runtime is stopped.
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
    /// # Errors
    ///
    /// On error, the corresponding error is returned and the runtime is stopped.
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
        let worker = ctx.into_worker().inspect_err(|_| self.shutdown.stop())?;

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
        let managed = RespawnableHandle::spawn_managed(ctx, &self.shutdown)
            .inspect_err(|_| self.shutdown.stop())?;

        self.respawnables.push(managed);
        Ok(())
    }

    /// Blocks the calling thread until all the runtime's workers stop.
    ///
    /// This function also takes care of respawning panicked workers launched using [`RespawnableContext`]. See the [`health_check`] for more details.
    ///
    /// [`health_check`]: Self::health_check
    ///
    /// # Example
    ///
    /// ```
    /// # use std::time::{Duration, Instant};
    /// # use employees::*;
    /// struct Employee;
    ///
    /// impl Worker for Employee {
    ///     fn on_update(&mut self) -> ControlFlow {
    ///         // Let's simulate some work.
    ///         std::thread::sleep(Duration::from_secs(1));
    ///
    ///         ControlFlow::Break
    ///     }
    /// }
    ///
    /// let mut runtime = Runtime::new();
    /// let now = Instant::now();
    ///
    /// runtime.launch(Employee).unwrap();
    /// runtime.wait();
    ///
    /// assert!(now.elapsed() >= Duration::from_secs(1));
    /// ```
    #[inline]
    pub fn wait(&mut self) {
        // We need to manage respawnable workers until there's none left.
        while !self.respawnables.is_empty() {
            self.health_check();
            std::thread::sleep(Duration::from_micros(1));
        }

        // Then we join the other workers
        for thread in self.threads.drain(..) {
            let _ = thread.join();
        }
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
    /// ```
    #[inline]
    pub fn health_check(&mut self) {
        self.respawnables.iter_mut().for_each(|managed| {
            // TODO: Do something with the errors
            let _ = managed.respawn_if_panicked(&self.shutdown);
        });

        // Filter the handles that actually finished without panicking.
        self.respawnables = self
            .respawnables
            .drain(..)
            .filter(|handle| !handle.is_finished())
            .collect::<Vec<_>>();
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
        let thread = crate::utils::spawn_thread(worker, settings, cores, &self.shutdown)
            .inspect_err(|_| self.shutdown.stop())?;

        self.threads.push(thread);
        Ok(())
    }
}

impl Default for Runtime {
    #[inline]
    fn default() -> Self {
        Self {
            shutdown: Shutdown::new(),
            threads: Vec::new(),
            respawnables: Vec::new(),
            nested: false,
        }
    }
}

impl From<Shutdown> for Runtime {
    #[inline]
    fn from(shutdown: Shutdown) -> Self {
        Self {
            shutdown,
            threads: Vec::new(),
            respawnables: Vec::new(),
            nested: true,
        }
    }
}

impl Drop for Runtime {
    fn drop(&mut self) {
        if !self.nested {
            self.shutdown.stop()
        }

        self.wait()
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
    fn start_stop() {
        let mut rt = Runtime::new();

        rt.launch(TestWorker)
            .expect("failed to launch the test actor");
        std::thread::sleep(Duration::from_millis(500));
    }

    #[test]
    fn wait() {
        let mut rt = Runtime::new();
        let now = Instant::now();
        let timeout = Duration::from_millis(500);

        rt.launch(TestTimedWorker::new(timeout))
            .expect("failed to launch the test actor");

        rt.wait();
        assert!(now.elapsed() > timeout);
    }

    #[test]
    fn pinned_actor() {
        let mut rt = Runtime::new();
        let core_id = rand::thread_rng().gen_range(0..5);

        rt.launch_pinned(TestPinnedWorker::new(core_id), [core_id])
            .expect("failed to launch the test actor");
        std::thread::sleep(Duration::from_millis(1));
    }

    #[test]
    fn stop_on_err() {
        let mut rt = Runtime::new();
        let now = Instant::now();

        rt.launch_from_context(BadWorkerContext)
            .expect_err("launching this worker should fail");
        rt.wait();
        assert!(now.elapsed() < Duration::from_millis(500));
    }
}
