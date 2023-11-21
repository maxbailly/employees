use std::time::Duration;

use crossbeam_channel::{unbounded, Receiver, Sender, TryRecvError};

use employees::{Connect, Context, ControlFlow, Error, Register, Runtime, Shutdown, Worker};

/* ---------- */

pub(crate) struct Consumers {
    nb_cons: u8,
    from_producers: Receiver<u32>,
}

impl Worker for Consumers {
    fn run(&mut self, shutdown: Shutdown) {
        let mut runtime = Runtime::nested(shutdown);

        for nth in 0..self.nb_cons {
            let cons = Consumer::new(self.from_producers.clone());
            if let Err(err) = runtime.launch(cons) {
                println!("failed to launch #{nth} consumer: {err:#}")
            }
        }
    }
}

/* ---------- */

pub(crate) struct ConsumersContext {
    nb_cons: u8,
    to_consumers: Sender<u32>,
    from_producers: Receiver<u32>,
}

impl ConsumersContext {
    pub(crate) fn new(nb_cons: u8) -> Self {
        let (to_consumers, from_producers) = unbounded();

        Self {
            nb_cons,
            to_consumers,
            from_producers,
        }
    }
}

impl Context for ConsumersContext {
    type Target = Consumers;

    fn into_worker(self) -> Result<Self::Target, Error> {
        Ok(Consumers {
            nb_cons: self.nb_cons,
            from_producers: self.from_producers,
        })
    }
}

impl Register for ConsumersContext {
    type Endpoint = Sender<u32>;

    fn register(&mut self, other: &mut impl Connect<Self>) {
        other.on_connection(self.to_consumers.clone())
    }
}

/* ---------- */

struct Consumer {
    from_producers: Receiver<u32>,
}

impl Consumer {
    fn new(from_producers: Receiver<u32>) -> Self {
        Self { from_producers }
    }
}

impl Worker for Consumer {
    fn on_update(&mut self) -> ControlFlow {
        match self.from_producers.try_recv() {
            Ok(msg) => println!("recv'd {msg} from a producer"),
            Err(TryRecvError::Empty) => (),
            Err(err) => println!("failed to recv a msg from producers: {err}"),
        }

        std::thread::sleep(Duration::from_millis(10));

        ControlFlow::Continue
    }
}
