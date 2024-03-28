use std::marker::PhantomData;
use std::thread::{Scope, ScopedJoinHandle};

use crate::settings::Settings;
use crate::utils::{Nested, Root, Shutdown, Type};
use crate::worker::Worker;
use crate::{Context, Error, RespawnableContext};

/* ---------- */

/// A runtime to manage [`Workers`] scoped threads.
///
/// [`Workers`]: crate::Worker
pub struct ScopedRuntime<'scope, 'env, T: Type> {
    scope: &'scope Scope<'scope, 'env>,

    shutdown: Shutdown,
    threads: Vec<ScopedJoinHandle<'scope, ()>>,
    managed: Vec<RespawnableScopedHandle<'scope, 'env>>,

    _type: PhantomData<T>,
}

impl<'scope, 'env> ScopedRuntime<'scope, 'env, Root> {
    /// Returns a new runtime bound to the `scope`.
    #[inline]
    pub fn new(scope: &'scope Scope<'scope, 'env>) -> Self {
        Self {
            scope,
            threads: Vec::new(),
            managed: Vec::new(),
            shutdown: Shutdown::new(),
            _type: PhantomData,
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

    /// Forces workers to leave their main loop.
    ///
    /// This function doesn't wait for thread completion.
    #[inline]
    pub fn stop(&self) {
        self.shutdown.stop()
    }
}

impl<'scope, 'env> ScopedRuntime<'scope, 'env, Nested> {
    /// Returns a new scoped runtime whose stopping condition is controlled by the "parent" runtime
    /// from which `shutdown` is originates.
    ///
    /// This allows users to spawn runtimes in workers without caring about the shutdown.
    /// Thus, the [`enable_graceful_shutdown`] and [`stop`] functions can't be called on a nested runtime.
    ///
    /// [`enable_graceful_shutdown`]: ScopedRuntime::enable_graceful_shutdown
    /// [`stop`]: ScopedRuntime::stop
    pub fn nested(scope: &'scope Scope<'scope, 'env>, shutdown: Shutdown) -> Self {
        Self {
            scope,
            shutdown,
            threads: Vec::new(),
            managed: Vec::new(),
            _type: PhantomData,
        }
    }
}

impl<'scope, 'env, T: Type> ScopedRuntime<'scope, 'env, T> {
    /// Runs an [`Worker`] in a new thread.
    ///
    /// Similar to the [`Runtime::launch`] function, see its documentation for more details.
    ///
    /// [`Runtime::launch`]: crate::Runtime::launch
    #[inline]
    pub fn launch<W: Worker + 'env>(&mut self, worker: W) -> Result<(), Error> {
        self.inner_spawn_thread(worker, Settings::default(), None::<Vec<_>>)
    }

    /// Runs an [`Worker`] in a new thread.
    ///
    /// Similar to the [`Runtime::launch_with_settings`] function, see its documentation for more details.
    ///
    /// [`Runtime::launch_with_settings`]: crate::Runtime::launch_with_settings
    #[inline]
    pub fn launch_with_settings<W: Worker + 'env>(
        &mut self,
        worker: W,
        settings: Settings,
    ) -> Result<(), Error> {
        self.inner_spawn_thread(worker, settings, None::<Vec<_>>)
    }

    /// Runs an [`Worker`] in a new thread.
    ///
    /// Similar to the [`Runtime::launch_pinned`] function, see its documentation for more details.
    ///
    /// [`Runtime::launch_pinned`]: crate::Runtime::launch_pinned
    #[inline]
    pub fn launch_pinned<W, C>(&mut self, worker: W, cores: C) -> Result<(), Error>
    where
        W: Worker + 'env,
        C: AsRef<[usize]> + Send + 'env,
    {
        self.launch_pinned_with_settings(worker, cores, Settings::default())
    }

    /// Runs an [`Worker`] in a new thread.
    ///
    /// Similar to the [`Runtime::launch_pinned_with_settings`] function, see its documentation for more details.
    ///
    /// [`Runtime::launch_pinned_with_settings`]: crate::Runtime::launch_pinned_with_settings
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

    /// Runs a [`Worker`] built from a [`Context`] that can be respawned if it panics.
    ///
    /// Similar to the [`Runtime::launch_from_context`] function, see its documentation for more details.
    ///
    /// [`Runtime::launch_from_context`]: crate::Runtime::launch_from_context
    #[inline]
    pub fn launch_from_context<W, C>(&mut self, ctx: C) -> Result<(), Error>
    where
        W: Worker + 'env,
        C: Context<Target = W>,
    {
        let settings = ctx.settings();
        let cores = ctx.core_pinning();
        let worker = ctx.into_worker()?;

        self.inner_spawn_thread(worker, settings, cores)
    }

    /// Runs a [`Worker`] built from a [`RespawnableContext`] that can be respawned if it panics.
    ///
    /// Similar to the [`Runtime::launch_respawnable`] function, see its documentation for more details.
    ///
    /// [`Runtime::launch_respawnable`]: crate::Runtime::launch_respawnable
    #[inline]
    pub fn launch_respawnable<R>(&mut self, ctx: R) -> Result<(), Error>
    where
        R: RespawnableContext<'env> + 'env,
    {
        let managed = RespawnableScopedHandle::spawn_managed(self.scope, ctx, &self.shutdown)?;

        self.managed.push(managed);
        Ok(())
    }

    /// Checks all respawnable [`Workers`], respawning the ones that panicked.
    ///
    /// Similar to the [`Runtime::health_check`] function, see its documentation for more details.
    ///
    /// [`Workers`]: crate::Worker
    /// [`Runtime::health_check`]: crate::Runtime::launch_respawnable
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

impl<T: Type> Drop for ScopedRuntime<'_, '_, T> {
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
        let worker = ctx.boxed_worker()?;

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
            let worker = self.context.boxed_worker()?;

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
    use std::time::{Duration, Instant};

    use rand::Rng;

    use super::*;
    use crate::test_utils::*;

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

            rt.launch(TestWorker)
                .expect("failed to launch the test actor");
            std::thread::sleep(Duration::from_millis(500));
            rt.stop();
        })
    }

    #[test]
    fn drop() {
        std::thread::scope(|scope| {
            let mut rt = ScopedRuntime::new(scope);
            let now = Instant::now();
            let timeout = Duration::from_millis(500);

            rt.launch(TestTimedWorker::new(timeout))
                .expect("failed to launch the test actor");

            std::mem::drop(rt);
            assert!(now.elapsed() > timeout);
        })
    }

    #[test]
    fn pinned_actor() {
        std::thread::scope(|scope| {
            let mut rt = ScopedRuntime::new(scope);
            let core_id = rand::thread_rng().gen_range(0..5);

            rt.launch_pinned(TestPinnedWorker::new(core_id), [core_id])
                .expect("failed to launch the test actor");
            std::thread::sleep(Duration::from_millis(1));
            rt.stop();
        })
    }
}
