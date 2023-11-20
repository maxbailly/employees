use std::time::{Duration, Instant};

use employees::*;

/* ---------- */

struct PanickingActor<'a> {
    name: &'a str,
}

impl Worker for PanickingActor<'_> {
    fn on_start(&mut self) {
        println!("[ACTOR] starting the panicking actor {}", self.name)
    }

    fn on_update(&mut self) -> ControlFlow {
        std::thread::sleep(Duration::from_secs(1));
        panic!("[ACTOR] the actor {} panics", self.name)
    }
}

impl Drop for PanickingActor<'_> {
    fn drop(&mut self) {
        println!("[ACTOR] dropping the panicking actor {}", self.name)
    }
}

/* ---------- */

struct PanickingActorContext<'a> {
    name: &'a str,
}

impl<'a> RespawnableContext<'a> for PanickingActorContext<'a> {
    fn boxed_actor(&self) -> Result<Box<dyn Worker + 'a>, Error> {
        Ok(Box::new(PanickingActor { name: self.name }))
    }
}

/* ---------- */

fn main() {
    let name = String::from("toto");
    let context = PanickingActorContext {
        name: name.as_str(),
    };

    std::thread::scope(|scope| {
        let mut runtime = ScopedRuntime::new(scope);

        runtime.enable_graceful_shutdown();
        runtime
            .launch_respawnable(context)
            .expect("[MAIN] failed to launch the running actor");

        let timeout = Duration::from_secs(5);
        let now = Instant::now();

        while now.elapsed() < timeout {
            std::thread::sleep(Duration::from_millis(100));
            println!("[MAIN] running health check");
            runtime.health_check();
        }

        runtime.stop()
    });
}
