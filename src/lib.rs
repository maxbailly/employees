//! A small and lightweight crate to hide all the boilerplate when working with threads.
//!
//! # Philosophy
//! This crate sees threads as unique entities called `actors` which live as long as the program live.
//!
//! One may notice many similarities with async `tasks` but there is one major difference.
//! While `tasks` are designed to be short and non-blocking pieces of concurrent code, `actors` on the other hand
//! are the complete opposite:
//! - they are designed to execute the concurrent code many time in an loop (e.g. an actor polling a TCP socket)
//! - they can block on operations as long as they (mostly) want without impacting other `actors`.
//!
//! Some similarities exist between a full-blown Entity Component System and this crate.
//! In some regards, this crate can be viewed as an ECS without the C part.

mod error;
mod runtime;
mod scoped_runtime;
mod service;
mod settings;
#[cfg(feature = "timing")]
pub mod timers;
mod utils;
mod worker;

pub use error::*;
pub use runtime::*;
pub use scoped_runtime::*;
pub use service::*;
pub use utils::*;
pub use worker::*;
