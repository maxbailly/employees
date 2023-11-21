use std::marker::PhantomData;
use std::thread::JoinHandle;

use crate::actor::{Context, RespawnableContext, Worker};
use crate::settings::Settings;
use crate::utils::Shutdown;
use crate::Error;

/* ---------- */

/// A runtime that manages [`Actor`]s threads.
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

    /// Stops the runtime, asking for all running actors to stop their loops.
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
    /// Runs an [`Actor`] in a new thread.
    #[inline]
    pub fn launch<A: Worker + 'static>(&mut self, actor: A) -> Result<(), Error> {
        self.launch_with_settings(actor, Settings::default())
    }

    /// Runs an [`Actor`] in a new configured thread.
    #[inline]
    pub fn launch_with_settings<A: Worker + 'static>(
        &mut self,
        mut actor: A,
        settings: Settings,
    ) -> Result<(), Error> {
        let shutdown = self.shutdown.clone();
        let thread = settings
            .into_inner()
            .spawn(move || actor.run(shutdown))
            .map_err(Error::Thread)?;

        self.threads.push(thread);

        Ok(())
    }

    /// Runs an [`Actor`] in a new thread where its thread is pinned to given cpu cores.
    #[inline]
    pub fn launch_pinned<A, C>(&mut self, actor: A, core_ids: C) -> Result<(), Error>
    where
        A: Worker + 'static,
        C: AsRef<[usize]> + Send + 'static,
    {
        self.launch_pinned_with_settings(actor, core_ids, Settings::default())
    }

    /// Runs an [`Actor`] in a new configured thread where its thread is pinned to given cpu cores.
    #[inline]
    pub fn launch_pinned_with_settings<A, C>(
        &mut self,
        mut actor: A,
        core_ids: C,
        settings: Settings,
    ) -> Result<(), Error>
    where
        A: Worker + 'static,
        C: AsRef<[usize]> + Send + 'static,
    {
        let shutdown = self.shutdown.clone();
        let thread = settings
            .into_inner()
            .spawn(move || {
                let _ = affinity::set_thread_affinity(core_ids);
                actor.run(shutdown)
            })
            .map_err(Error::Thread)?;

        self.threads.push(thread);

        Ok(())
    }

    /// Runs an [`Actor`] built from a context in a new thread.
    #[inline]
    pub fn launch_from_context<A, B>(&mut self, ctx: B) -> Result<(), Error>
    where
        A: Worker + 'static,
        B: Context<Target = A>,
    {
        let settings = ctx.settings();
        let cores = ctx.core_pinning();
        let actor = ctx.into_actor()?;

        match cores {
            Some(cores) => self.launch_pinned_with_settings(actor, cores, settings),
            None => self.launch_with_settings(actor, settings),
        }
    }

    #[inline]
    pub fn launch_respawnable<M>(&mut self, ctx: M) -> Result<(), Error>
    where
        M: RespawnableContext<'static> + 'static,
    {
        let managed = RespawnableHandle::spawn_managed(ctx, &self.shutdown)?;

        self.respawnable.push(managed);
        Ok(())
    }

    #[inline]
    pub fn health_check(&mut self) {
        self.respawnable.iter_mut().for_each(|managed| {
            // TODO: Do something with the errors
            let _ = managed.respawn_if_panicked(&self.shutdown);
        })
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
    fn spawn_managed(
        ctx: impl RespawnableContext<'static> + 'static,
        shutdown: &Shutdown,
    ) -> Result<Self, Error> {
        let cores = ctx.core_pinning();
        let settings = ctx.settings();
        let mut actor = ctx.boxed_actor()?;
        let shutdown = shutdown.clone();

        let thread = settings
            .into_inner()
            .spawn(move || {
                if let Some(cores) = cores {
                    let _ = affinity::set_thread_affinity(cores);
                }
                actor.run(shutdown);
            })
            .map_err(Error::Thread)?;

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
            let shutdown = shutdown.clone();
            let cores = self.context.core_pinning();
            let settings = self.context.settings();
            let mut actor = self.context.boxed_actor()?;

            let thread = settings
                .into_inner()
                .spawn(move || {
                    if let Some(cores) = cores {
                        let _ = affinity::set_thread_affinity(cores);
                    }
                    actor.run(shutdown)
                })
                .map_err(Error::Thread)?;

            self.handle = Some(thread);
        }

        Ok(())
    }
}

/* ---------- */

pub trait Type {}

#[derive(Debug)]
pub enum Root {}
impl Type for Root {}

#[derive(Debug)]
pub enum Nested {}
impl Type for Nested {}
