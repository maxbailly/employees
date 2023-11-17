use std::time::{Duration, Instant};

use actor_pattern::*;

/* ---------- */

struct PanickingActor;

impl Actor for PanickingActor {
    fn on_start(&mut self) {
        println!("[ACTOR] starting the panicking actor")
    }

    fn on_update(&mut self) -> ControlFlow {
        std::thread::sleep(Duration::from_secs(1));
        panic!("[ACTOR] the actor panics")
    }
}

impl Drop for PanickingActor {
    fn drop(&mut self) {
        println!("[ACTOR] dropping the panicking actor")
    }
}

/* ---------- */

struct PanickingActorContext;

impl RespawnableContext for PanickingActorContext {
    fn boxed_actor(&self) -> Result<Box<dyn Actor>, Error> {
        Ok(Box::new(PanickingActor))
    }
}

/* ---------- */

fn main() {
    let mut runtime = Runtime::new();

    runtime.enable_graceful_shutdown();
    runtime
        .launch_respawnable(PanickingActorContext)
        .expect("[MAIN] failed to launch the running actor");

    let timeout = Duration::from_secs(5);
    let now = Instant::now();

    while now.elapsed() < timeout {
        std::thread::sleep(Duration::from_millis(100));
        println!("[MAIN] running health check");
        runtime.health_check();
    }

    runtime.stop()
}
