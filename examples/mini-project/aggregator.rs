use std::time::Duration;

use crossbeam_channel::{Receiver, RecvTimeoutError, Sender};
use employees::minuteurs::*;
use employees::{Connect, Context, Error, Register, Shutdown, Worker};

use crate::messages::AggregatorMessage;

/* ---------- */

pub(crate) struct Aggregator {
    from_workers: Receiver<AggregatorMessage>,
    period: Watcher,
}

impl Worker for Aggregator {
    fn run(&mut self, stop: Shutdown) {
        while stop.is_running() {
            match self.from_workers.recv_timeout(Duration::from_millis(10)) {
                Ok(_) => println!("recv'd data from workers"),
                Err(RecvTimeoutError::Disconnected) => {
                    println!("all workers are stopped, stopping the aggregator...");
                    break;
                }
                _ => (),
            }

            if self.period.has_ticked() {
                println!("do something with the data collected from workers");
            }
        }
    }
}

/* ---------- */

pub(crate) struct AggregatorContext {
    from_workers: Receiver<AggregatorMessage>,
    to_aggregator: Sender<AggregatorMessage>,
    period: Option<Watcher>,
}

impl AggregatorContext {
    pub(crate) fn new() -> Self {
        let (to_aggregator, from_workers) = crossbeam_channel::unbounded();

        Self {
            from_workers,
            to_aggregator,
            period: None,
        }
    }
}

impl Context for AggregatorContext {
    type Target = Aggregator;

    fn into_worker(self) -> Result<Self::Target, Error> {
        let period = self.period.ok_or(Error::context("period"))?;

        Ok(Aggregator {
            period,
            from_workers: self.from_workers,
        })
    }
}

impl Register for AggregatorContext {
    type Endpoint = Sender<AggregatorMessage>;

    fn register(&mut self, other: &mut impl Connect<Self>) {
        other.on_connection(self.to_aggregator.clone())
    }
}

impl Connect<Timer> for AggregatorContext {
    fn on_connection(&mut self, recver: Watcher) {
        let _ = self.period.insert(recver);
    }
}
