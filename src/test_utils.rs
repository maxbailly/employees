use std::time::{Duration, Instant};

use crate::{Context, ControlFlow, Error, Worker};

/* ---------- */

pub(crate) struct TestWorker;

impl Worker for TestWorker {
    fn on_update(&mut self) -> ControlFlow {
        std::thread::sleep(Duration::from_millis(1));
        ControlFlow::Continue
    }
}

/* ---------- */

pub(crate) struct TestTimedWorker {
    timeout: Duration,
    now: Instant,
}

impl TestTimedWorker {
    pub(crate) fn new(timeout: Duration) -> Self {
        Self {
            timeout,
            now: Instant::now(),
        }
    }
}

impl Worker for TestTimedWorker {
    fn on_start(&mut self) {
        self.now = Instant::now();
    }

    fn on_update(&mut self) -> ControlFlow {
        if self.now.elapsed() >= self.timeout {
            return ControlFlow::Break;
        }

        ControlFlow::Continue
    }
}

/* ---------- */

pub(crate) struct TestPinnedWorker(usize);

impl TestPinnedWorker {
    pub(crate) fn new(core: usize) -> Self {
        Self(core)
    }
}

impl Worker for TestPinnedWorker {
    fn on_update(&mut self) -> ControlFlow {
        assert_eq!(self.0, affinity::get_thread_affinity().unwrap()[0]);
        ControlFlow::Break
    }
}

/* ---------- */

pub(crate) struct BadWorker;

impl Worker for BadWorker {
    fn on_update(&mut self) -> ControlFlow {
        std::thread::sleep(Duration::from_millis(500));
        ControlFlow::Break
    }
}

pub(crate) struct BadWorkerContext;

impl Context for BadWorkerContext {
    type Target = BadWorker;

    fn into_worker(self) -> Result<Self::Target, Error> {
        Err(Error::context("bad context"))
    }
}
