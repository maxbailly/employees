//! Re-exports and implementations for types from the [`minuteurs`] crate.
//!
//! [`minuteurs`]: <https://docs.rs/minuteurs/latest/minuteurs/>

use crate::minuteurs::{Timer, Watcher};
use crate::{Connect, ControlFlow, Register, Worker};

/* ---------- */

impl Worker for Timer {
    #[inline]
    fn on_update(&mut self) -> ControlFlow {
        self.tick();
        ControlFlow::Continue
    }
}

impl Register for Timer {
    type Endpoint = Watcher;

    #[inline]
    fn register(&mut self, client: &mut impl Connect<Self>) {
        client.on_connection(self.watcher())
    }
}
