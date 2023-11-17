mod consumers;
mod producer;

use consumers::*;
use employees::{Register, Runtime};
use producer::*;

/* ---------- */

fn main() {
    let mut runtime = Runtime::new();
    let mut producers = ProducersContext::new(2);
    let mut consumers = ConsumersContext::new(16);

    consumers.register(&mut producers);

    runtime.enable_graceful_shutdown();
    runtime
        .launch_from_context(producers)
        .expect("failed to launch the producers");
    runtime
        .launch_from_context(consumers)
        .expect("failed to launch the consumers");
}
