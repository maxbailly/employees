use crossbeam_channel::Sender;
use employees::{Connect, Context, ControlFlow, Error, Worker};
use minuteurs::{Timer, Watcher};

use crate::aggregator::AggregatorContext;
use crate::messages::AggregatorMessage;

/* ---------- */

pub(crate) struct MyWorker {
    period: Watcher,
    to_aggreg: Sender<AggregatorMessage>,
}

impl Worker for MyWorker {
    fn on_update(&mut self) -> ControlFlow {
        if self.period.has_ticked() {
            println!("sending data to the aggregator");

            if let Err(err) = self.to_aggreg.send(AggregatorMessage::Data) {
                println!("failed to data to the aggregator: {err}")
            }
        }

        ControlFlow::Continue
    }
}

/* ---------- */

#[derive(Default)]
pub(crate) struct MyWorkerContext {
    period: Option<Watcher>,
    to_aggreg: Option<Sender<AggregatorMessage>>,
}

impl Context for MyWorkerContext {
    type Target = MyWorker;

    fn into_actor(self) -> Result<Self::Target, Error> {
        let period = self
            .period
            .ok_or(Error::InvalidContext("period".to_owned()))?;

        let to_aggreg = self
            .to_aggreg
            .ok_or(Error::InvalidContext("to_aggreg".to_owned()))?;

        Ok(MyWorker { period, to_aggreg })
    }
}

impl Connect<Timer> for MyWorkerContext {
    fn on_connection(&mut self, recver: Watcher) {
        let _ = self.period.insert(recver);
    }
}

impl Connect<AggregatorContext> for MyWorkerContext {
    fn on_connection(&mut self, endpoint: Sender<AggregatorMessage>) {
        let _ = self.to_aggreg.insert(endpoint);
    }
}
