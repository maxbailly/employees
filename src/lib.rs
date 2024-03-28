//! A small and lightweight crate to hide most of the burden of setting up threads.
//!
//! # Philosophy
//!
//! This crate sees threads as unique entities called `workers` which live as long as the program lives.
//!
//! One may notice many similarities with async `tasks` but there is one major difference.
//! While `tasks` are designed to be short and non-blocking pieces of concurrent code running in async runtimes,
//! `workers` on the other hand are the complete opposite:
//! - they run on their own OS thread.
//! - they are designed to run for a very long time (mostly for the lifetime of the program).
//! - they can block on operations as long as they (mostly) want without impacting other `workers`.
//!
//! Some similarities exist between a full-blown Entity Component System and this crate.
//! In some regards, this crate can be viewed as an ECS without the C part.
//!
//! # Usage
//!
//! Here's a small example that spawns a worker that prints "Hello, World!" every 100ms for 1 second.
//!
//! ```
//! # use employees::{Runtime, Worker, ControlFlow};
//! # use std::time::Duration;
//! struct WorkerThatPrints;
//! impl Worker for WorkerThatPrints {
//!     fn on_update(&mut self) -> ControlFlow {
//!         println!("Hello, World!");
//!         std::thread::sleep(Duration::from_millis(100));
//!         ControlFlow::Continue
//!     }
//! }
//!
//! let mut runtime = Runtime::new();
//!
//! runtime.launch(WorkerThatPrints);
//! std::thread::sleep(Duration::from_secs(1));
//! ```
//!
//! # Features
//!
//! [`Runtimes`] and [`Workers`] comes with a set of various features and helpers to setup and run them.
//!
//! [`Runtimes`]: crate::Runtime
//! [`Workers`]: crate::Worker
//!
//! ## Runtimes and non-`'static` things
//!
//! [`Runtimes`] need `'static` lifetimes, therefore the following example won't compile.
//!
//! [`Runtimes`]: crate::Runtime
//!
//! ```compile_fail
//! # use employees::{Runtime, Worker, ControlFlow};
//! struct WorkerThatPrints<'a>(&'a str);
//! impl Worker for WorkerThatPrints<'_> {
//!     fn on_update(&mut self) -> ControlFlow {
//!         println!("Hello, {}!", self.0);
//!         ControlFlow::Continue
//!     }
//! }
//!
//! let name = String::from("Alice");
//! let worker = WorkerThatPrints(&name);
//!
//! let mut runtime = Runtime::new();
//! runtime.launch(worker); // worker isn't 'static!
//! ```
//!
//! Fortunately, this crate provides [`ScopedRuntimes`]. They are a 1 to 1 implementation of classic [`Runtimes`] except that they need a scope.
//!
//! [`Runtimes`]: crate::Runtime
//! [`ScopedRuntimes`]: crate::ScopedRuntime
//!
//! ```
//! # use employees::{ScopedRuntime, Worker, ControlFlow};
//! # struct WorkerThatPrints<'a>(&'a str);
//! # impl Worker for WorkerThatPrints<'_> {
//! #     fn on_update(&mut self) -> ControlFlow {
//! #         println!("Hello, {}!", self.0);
//! #         ControlFlow::Break
//! #     }
//! # }
//! let name = String::from("Alice");
//! let worker = WorkerThatPrints(&name);
//!
//! std::thread::scope(|scope| {
//!     // Let's create a scoped runtime
//!     let mut runtime = ScopedRuntime::new(scope);
//!
//!     runtime.launch(worker); // Now, that works!
//! })
//! ```
//!
//! ## Configuring the threads
//!
//! Workers threads can be configured via the [`Settings`] type with users can use it to set a thread's stack size and name.
//! Then, by passing the settings alongside the actor using the [`Runtime::launch_with_settings`] function, the thread will
//! be spawned with the specified settings.
//!
//! Users can also set the thread's affinity by passing a list of CPU IDs to the [`Runtime::launch_pinned`] function.
//!
//! The [`Runtime::launch_pinned_with_settings`] can do both.
//!
//! ```
//! # use employees::{Runtime, Worker, ControlFlow, Settings};
//! # struct WorkerThatPrints;
//! # impl Worker for WorkerThatPrints {
//! #     fn on_update(&mut self) -> ControlFlow {
//! #         println!("Hello, World!");
//! #         ControlFlow::Break
//! #     }
//! # }
//! let mut runtime = Runtime::new();
//! let settings = Settings::new().name("worker");
//!
//! // The thread will be named "worker" and pinned to the CPUs #0 and #1
//! runtime.launch_pinned_with_settings(WorkerThatPrints, [0,1], settings);
//! ```
//!
//! ## Contextes
//!
//! Users may want to configure their workers via a builder pattern before spawning them in a runtime,
//! somewhat like how the [`std::process::Command`] type works. This can be done easily giving a type that implements
//! the [`Context`] trait to the [`Runtime::launch_from_context`] function. Contextes also provide thread settings.
//!
//! ```
//! # use employees::{Runtime, Worker, ControlFlow, Settings, Context, Error};
//! # struct WorkerThatPrints(String);
//! # impl Worker for WorkerThatPrints {
//! #     fn on_update(&mut self) -> ControlFlow {
//! #         println!("Hello, {}!", self.0);
//! #         ControlFlow::Break
//! #     }
//! # }
//! struct WorkerContext {
//!     name: String
//! }
//!
//! impl Context for WorkerContext {
//!     type Target = WorkerThatPrints;
//!
//!     fn into_worker(self) -> Result<Self::Target, Error> {
//!         Ok(WorkerThatPrints(self.name))
//!     }
//!
//!     fn settings(&self) -> Settings {
//!         Settings::new().name("worker")
//!     }
//!
//!     fn core_pinning(&self) -> Option<Vec<usize>> {
//!         Some(vec![0,1])
//!     }
//! }
//!
//! let mut runtime = Runtime::new();
//! let context = WorkerContext { name: String::from("Alice") };
//!
//! runtime.launch_from_context(context);
//! ```
//!
//! ## Respawning workers that panicked
//!
//! The runtimes allow respawning workers that panicked. This can be achieved by implementing the [`RespawnableContext`] trait
//! for a type a passing that type to the [`Runtime::launch_respawnable`] function.
//!
//! By calling the [`Runtime::health_check`] function, a runtime will check every respawnable workers that panicked and will respawn
//! it using their respective contextes.
//!
//! ```
//! # use employees::{Runtime, Worker, ControlFlow, RespawnableContext, Error};
//! # use std::time::Duration;
//! // A worker that panic some time after being spawned...
//! struct PanickingWorker;
//! impl Worker for PanickingWorker {
//!     fn on_update(&mut self) -> ControlFlow {
//!         std::thread::sleep(Duration::from_secs(1));
//!         panic!("panicking!")
//!     }
//! }
//!
//! // ... and its context.
//! struct WorkerContext;
//! impl RespawnableContext<'_> for WorkerContext {
//!     fn boxed_worker(&self) -> Result<Box<dyn Worker>, Error> {
//!         Ok(Box::new(PanickingWorker))
//!     }
//! }
//!
//! let mut runtime = Runtime::new();
//! runtime.launch_respawnable(WorkerContext);
//!
//! std::thread::sleep(Duration::from_secs(1));
//! runtime.health_check();
//! # std::thread::sleep(Duration::from_millis(500));
//! # runtime.wait();
//! ```
//!
//! ## Inter-workers communication
//!
//! This crate exposes two traits, [`Register`] and [`Connect`], enabling the communication between two workers.
//! Those traits are channel agnostics and direction: virtually anything that can send or receive data or share states
//! can be used as a [`Register::Endpoint`].
//!
//! ```
//! # use std::sync::mpsc::{self, Sender, Receiver};
//! # use employees::{Runtime, Worker, ControlFlow, Error, Register, Connect};
//! # use std::time::Duration;
//! // A producer worker.
//! #[derive(Default)]
//! struct Producer {
//!     sender: Option<Sender<u8>>,
//! }
//!
//! impl Worker for Producer {
//!     fn on_update(&mut self) -> ControlFlow {
//!         std::thread::sleep(Duration::from_millis(100));
//!
//!         if let Some(sender) = self.sender.as_ref() {
//!             sender.send(1).unwrap();
//!         }
//!
//!         ControlFlow::Continue
//!     }
//! }
//!
//! impl Connect<Consumer> for Producer {
//!     fn on_connection(&mut self, endpoint: Sender<u8>) {
//!         self.sender = Some(endpoint)
//!     }
//! }
//!
//! // A consumer worker
//! struct Consumer {
//!     sender: Sender<u8>,
//!     recver: Receiver<u8>,
//!     count: u8,
//! }
//!
//! impl Consumer {
//!     fn new() -> Self {
//!         let (sender, recver) = mpsc::channel();
//!
//!         Self {
//!             sender,
//!             recver,
//!             count: 0,
//!         }
//!     }
//! }
//!
//! impl Worker for Consumer {
//!     fn on_update(&mut self) -> ControlFlow {
//!         let val = self.recver.recv().unwrap();
//!
//!         self.count += val;
//!         ControlFlow::Continue
//!     }
//! }
//!
//! impl Register for Consumer {
//!     type Endpoint = Sender<u8>;
//!
//!     fn register(&mut self, other: &mut impl Connect<Self>) {
//!         other.on_connection(self.sender.clone())
//!     }
//! }
//!
//! let mut consumer = Consumer::new();
//! let mut prod1 = Producer::default();
//! let mut prod2 = Producer::default();
//! let mut prod3 = Producer::default();
//!
//! // Let's connect everything
//! consumer.register(&mut prod1);
//! consumer.register(&mut prod2);
//! consumer.register(&mut prod3);
//!
//! // Launch the workers in a runtime
//! let mut runtime = Runtime::new();
//! runtime.launch(consumer);
//! runtime.launch(prod1);
//! runtime.launch(prod2);
//! runtime.launch(prod3);
//!
//! # std::thread::sleep(Duration::from_secs(1));
//! ```
//!
//! ## Timers
//!
//! This crate re-exports all types from the [`minuteurs`] crate and implements the [`Worker`] and [`Register`] traits on
//! the [`Timer`] type, allowing it to be used in runtimes.
//!
//! Requires the `timers` feature.
//!
//! [`minuteurs`]: <https://docs.rs/minuteurs/latest/minuteurs/>
//! [`Timer`]: minuteurs::Timer

#![warn(missing_docs)]

mod error;
mod runtime;
mod scoped_runtime;
mod service;
mod settings;
#[cfg(test)]
mod test_utils;
#[cfg(feature = "timing")]
pub mod timers;
mod utils;
mod worker;

pub use error::*;
pub use runtime::*;
pub use scoped_runtime::*;
pub use service::*;
pub use settings::*;
pub use utils::*;
pub use worker::*;
