use std::marker::PhantomData;
use std::thread::JoinHandle;

use crate::settings::Settings;
use crate::utils::{Nested, Root, Shutdown, Type};
use crate::worker::{Context, RespawnableContext, Worker};
use crate::Error;

/* ---------- */

/// A runtime that manages [`Worker`]s threads.
///
/// If the runtime is dropped, all threads are automatically joined.
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

    /// Stops the runtime, asking for all running workers to stop their loops.
    #[inline]
    pub fn stop(&self) {
        self.shutdown.stop()
    }
}

impl Runtime<Nested> {
    #[inline]
    pub fn nested(shutdown: Shutdown) -> Self {
        Self::from(shutdown)
    }
}

impl<T: Type> Runtime<T> {
    /// Runs a [`Worker`] in a new thread.
    #[inline]
    pub fn launch<W: Worker + 'static>(&mut self, worker: W) -> Result<(), Error> {
        self.inner_spawn_thread(worker, Settings::default(), None::<Vec<_>>)
    }

    /// Runs a [`Worker`] in a new configured thread.
    #[inline]
    pub fn launch_with_settings<W: Worker + 'static>(
        &mut self,
        worker: W,
        settings: Settings,
    ) -> Result<(), Error> {
        self.inner_spawn_thread(worker, settings, None::<Vec<_>>)
    }

    /// Runs a [`Worker`] in a new thread where its thread is pinned to given cpu cores.
    #[inline]
    pub fn launch_pinned<W, C>(&mut self, worker: W, cores: C) -> Result<(), Error>
    where
        W: Worker + 'static,
        C: AsRef<[usize]> + Send + 'static,
    {
        self.inner_spawn_thread(worker, Settings::default(), Some(cores))
    }

    /// Runs a [`Worker`] in a new configured thread where its thread is pinned to given cpu cores.
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

    /// Runs a [`Worker`] built from a context in a new thread.
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

    /// Runs a [`Worker`] that can be respawned if it panics.
    #[inline]
    pub fn launch_respawnable<C>(&mut self, ctx: C) -> Result<(), Error>
    where
        C: RespawnableContext<'static> + 'static,
    {
        let managed = RespawnableHandle::spawn_managed(ctx, &self.shutdown)?;

        self.respawnable.push(managed);
        Ok(())
    }

    /// Checks all respawnable [`Worker`]s, respawning the ones that panicked.
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
