mod aggregator;
mod http_server;
mod messages;
mod worker;

use std::time::Duration;

use employees::timers::*;
use employees::{Register, Runtime};

use crate::aggregator::AggregatorContext;
use crate::http_server::HttpServer;
use crate::worker::MyWorkerContext;

/* ---------- */

fn main() {
    const NB_WORKERS: usize = 10;

    let mut rt = Runtime::new();
    rt.enable_graceful_shutdown();

    let mut timer = Timer::new(Duration::from_secs(1));
    let mut aggregator = AggregatorContext::new();

    timer.register(&mut aggregator);

    for _ in 0..NB_WORKERS {
        let mut worker = MyWorkerContext::default();

        aggregator.register(&mut worker);
        timer.register(&mut worker);
        rt.launch_from_context(worker)
            .expect("failed to spawn the worker")
    }

    rt.launch_from_context(aggregator)
        .expect("failed to spawn the aggregator");
    rt.launch(HttpServer)
        .expect("failed to spawn the http server");
    rt.launch(timer).expect("failed to spawn the timer");
}
