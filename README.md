# employees

A small and lightweight crate to hide most of the burden of setting up threads.

# Philosophy

This crate sees threads as unique entities called `workers` which live as long as the program lives.

One may notice many similarities with async `tasks` but there is one major difference.
While `tasks` are designed to be short and non-blocking pieces of concurrent code running in async runtimes,
`workers` on the other hand are the complete opposite:
- they run on their own OS thread.
- they are designed to run for a very long time (mostly for the lifetime of the program).
- they can block on operations as long as they (mostly) want without impacting other `workers`.

Some similarities exist between a full-blown Entity Component System and this crate.
In some regards, this crate can be viewed as an ECS without the C part.

# Usage

Here's a small example that spawns a worker that prints "Hello, World!" every 100ms for 1 second.

```rust
struct WorkerThatPrints;
impl Worker for WorkerThatPrints {
    fn on_update(&mut self) -> ControlFlow {
        println!("Hello, World!");
        std::thread::sleep(Duration::from_millis(100));
        ControlFlow::Continue
    }
}

let mut runtime = Runtime::new();

runtime.launch(WorkerThatPrints);
std::thread::sleep(Duration::from_secs(1));
runtime.stop();
```

See [the full documentation](https://docs.rs/employees/latest/employees/) for more in depth details.

# License

Licensed under the terms of MIT license. See [LICENSE](LICENSE) for details.
