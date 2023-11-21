use std::thread::{Scope, ScopedJoinHandle};

use crate::settings::Settings;
use crate::utils::Shutdown;
use crate::worker::Worker;
use crate::{Context, Error, RespawnableContext};

/* ---------- */

/// A runtime to manage [`Actor`]s scoped threads.
pub struct ScopedRuntime<'scope, 'env> {
    scope: &'scope Scope<'scope, 'env>,

    threads: Vec<ScopedJoinHandle<'scope, ()>>,
    managed: Vec<RespawnableScopedHandle<'scope, 'env>>,

    shutdown: Shutdown,
}

impl<'scope, 'env> ScopedRuntime<'scope, 'env> {
    /// Returns a new runtime bound to the `scope`.
    #[inline]
    pub fn new(scope: &'scope Scope<'scope, 'env>) -> Self {
        Self {
            scope,
            threads: Vec::new(),
            managed: Vec::new(),
            shutdown: Shutdown::new(),
        }
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

    /// Runs an [`Actor`] in a new thread.
    #[inline]
    pub fn launch<W: Worker + 'env>(&mut self, worker: W) -> Result<(), Error> {
        self.inner_spawn_thread(worker, Settings::default(), None::<Vec<_>>)
    }

    /// Runs an [`Actor`] in a new configured thread.
    #[inline]
    pub fn launch_with_settings<W: Worker + 'env>(
        &mut self,
        worker: W,
        settings: Settings,
    ) -> Result<(), Error> {
        self.inner_spawn_thread(worker, settings, None::<Vec<_>>)
    }

    /// Runs an [`Actor`] in a new thread where its thread is pinned to given cpu cores.
    #[inline]
    pub fn launch_pinned<W, C>(&mut self, worker: W, cores: C) -> Result<(), Error>
    where
        W: Worker + 'env,
        C: AsRef<[usize]> + Send + 'env,
    {
        self.launch_pinned_with_settings(worker, cores, Settings::default())
    }

    /// Runs an [`Actor`] in a new configured thread where its thread is pinned to given cpu cores.
    #[inline]
    pub fn launch_pinned_with_settings<W, C>(
        &mut self,
        worker: W,
        cores: C,
        settings: Settings,
    ) -> Result<(), Error>
    where
        W: Worker + 'env,
        C: AsRef<[usize]> + Send + 'env,
    {
        self.inner_spawn_thread(worker, settings, Some(cores))
    }

    /// Runs an [`Actor`] built from a context in a new thread.
    #[inline]
    pub fn launch_from_context<W, C>(&mut self, ctx: C) -> Result<(), Error>
    where
        W: Worker + 'env,
        C: Context<Target = W>,
    {
        let settings = ctx.settings();
        let cores = ctx.core_pinning();
        let worker = ctx.into_actor()?;

        self.inner_spawn_thread(worker, settings, cores)
    }

    #[inline]
    pub fn launch_respawnable<R>(&mut self, ctx: R) -> Result<(), Error>
    where
        R: RespawnableContext<'env> + 'env,
    {
        let managed = RespawnableScopedHandle::spawn_managed(self.scope, ctx, &self.shutdown)?;

        self.managed.push(managed);
        Ok(())
    }

    #[inline]
    pub fn health_check(&mut self) {
        self.managed.iter_mut().for_each(|managed| {
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
        W: Worker + 'env,
        C: AsRef<[usize]> + Send + 'env,
    {
        let thread =
            crate::utils::spawn_scoped_thread(self.scope, worker, settings, cores, &self.shutdown)?;

        self.threads.push(thread);
        Ok(())
    }
}

impl Drop for ScopedRuntime<'_, '_> {
    fn drop(&mut self) {
        for thread in self.threads.drain(..) {
            let _ = thread.join();
        }

        for thread in self.managed.drain(..) {
            thread.join();
        }
    }
}

/* ---------- */

struct RespawnableScopedHandle<'scope, 'env> {
    scope: &'scope Scope<'scope, 'env>,
    handle: Option<ScopedJoinHandle<'scope, ()>>,
    context: Box<dyn RespawnableContext<'env> + 'env>,
}

impl<'scope, 'env> RespawnableScopedHandle<'scope, 'env> {
    #[inline]
    fn spawn_managed(
        scope: &'scope Scope<'scope, 'env>,
        ctx: impl RespawnableContext<'env> + 'env,
        shutdown: &Shutdown,
    ) -> Result<Self, Error> {
        let cores = ctx.core_pinning();
        let settings = ctx.settings();
        let worker = ctx.boxed_actor()?;

        let thread = crate::utils::spawn_scoped_thread(scope, worker, settings, cores, shutdown)?;

        Ok(Self {
            scope,
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
            let worker = self.context.boxed_actor()?;

            let thread =
                crate::utils::spawn_scoped_thread(self.scope, worker, settings, cores, shutdown)?;

            self.handle = Some(thread);
        }

        Ok(())
    }
}

/* ---------- */

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use rand::Rng;

    use super::*;
    use crate::worker::{ControlFlow, Worker};

    struct TestActor;
    impl Worker for TestActor {
        fn on_update(&mut self) -> ControlFlow {
            ControlFlow::Continue
        }
    }

    #[test]
    fn new_runtime() {
        std::thread::scope(|scope| {
            let rt = ScopedRuntime::new(scope);

            rt.stop()
        })
    }

    #[test]
    fn start_stop() {
        std::thread::scope(|scope| {
            let mut rt = ScopedRuntime::new(scope);

            rt.launch(TestActor)
                .expect("failed to launch the test actor");
            std::thread::sleep(Duration::from_millis(500));
            rt.stop();
        })
    }

    #[test]
    fn pinned_actor() {
        struct TestPinnedActor(usize);
        impl Worker for TestPinnedActor {
            fn on_update(&mut self) -> ControlFlow {
                assert_eq!(self.0, affinity::get_thread_affinity().unwrap()[0]);
                ControlFlow::Break
            }
        }

        std::thread::scope(|scope| {
            let mut rt = ScopedRuntime::new(scope);
            let core_id = rand::thread_rng().gen_range(0..5);

            rt.launch_pinned(TestPinnedActor(core_id), [core_id])
                .expect("failed to launch the test actor");
            std::thread::sleep(Duration::from_millis(1));
            rt.stop();
        })
    }
}
