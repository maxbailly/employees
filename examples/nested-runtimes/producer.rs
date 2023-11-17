use std::time::Duration;

use crossbeam_channel::Sender;
use employees::{Connect, Context, ControlFlow, Error, Runtime, Shutdown, Worker};
use rand::Rng;

use crate::consumers::ConsumersContext;

/* ---------- */

pub(crate) struct Producers {
    nb_prod: u8,
    to_consumers: Sender<u32>,
}

impl Worker for Producers {
    fn run(&mut self, shutdown: Shutdown) {
        let mut runtime = Runtime::nested(shutdown);

        println!("running {} producers", self.nb_prod);

        for nth in 0..self.nb_prod {
            let prod = Producer::new(self.to_consumers.clone());
            if let Err(err) = runtime.launch(prod) {
                println!("failed to launch #{nth} producer: {err:#}")
            }
        }
    }
}

/* ---------- */

pub(crate) struct ProducersContext {
    nb_prod: u8,
    to_consumers: Option<Sender<u32>>,
}

impl ProducersContext {
    pub(crate) fn new(nb_prod: u8) -> Self {
        Self {
            nb_prod,
            to_consumers: None,
        }
    }
}

impl Context for ProducersContext {
    type Target = Producers;

    fn into_actor(self) -> Result<Self::Target, Error> {
        let to_consumers = self
            .to_consumers
            .ok_or(Error::InvalidContext("to_consumers".to_owned()))?;

        Ok(Producers {
            nb_prod: self.nb_prod,
            to_consumers,
        })
    }
}

impl Connect<ConsumersContext> for ProducersContext {
    fn on_connection(&mut self, endpoint: Sender<u32>) {
        let _ = self.to_consumers.insert(endpoint);
    }
}

/* ---------- */

struct Producer {
    to_consumers: Sender<u32>,
}

impl Producer {
    fn new(to_consumers: Sender<u32>) -> Self {
        Self { to_consumers }
    }
}

impl Worker for Producer {
    fn on_update(&mut self) -> ControlFlow {
        let rand_data = rand::random();
        if let Err(err) = self.to_consumers.send(rand_data) {
            println!("error when sending some data to consumers: {err:#}")
        }

        let sleep_time = rand::thread_rng().gen_range(500..1000);
        std::thread::sleep(Duration::from_millis(sleep_time));

        ControlFlow::Continue
    }
}
