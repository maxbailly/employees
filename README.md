# `employees`

A small and lightweight crate to hide most of the burden of setting up long-living threads.

# Philosophy

This crate sees threads as unique entities called `workers` which may (or may not) live as long as the program lives. `Workers` are spawned in `Runtimes` that manage them.

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
```

See [the full documentation](https://docs.rs/employees/latest/employees/) for more in depth details.

# Should I use `employees`?

## Concurrency

`employees` is built with *CPU-bound concurrency* in mind.

"But wait, arent't async runtimes built for that purpose?" you might ask, and you'll be 100% right.

However, `async` has drawbacks which `employees` hasn't:

* async tasks must not be blocking,
* the famous [colored function problem](https://journal.stuffwithstuff.com/2015/02/01/what-color-is-your-function/).

That said, there are usecases where async runtime will be better such as I/O bound tasks (web servers, ect.) or concurrent tasks on single threaded platforms.

## Paralellism

Again, `employees` is built with *concurrency* in mind. While it is possible to build a work stealing thread pool from `employees` to parallelize some kind of computation, other crates such as [`rayon`](https://docs.rs/rayon/latest/rayon/) add a ton of utilities speficically made for that purpose which is why, for most paralellism usecases, `rayon`-like crates will be a better choice.

# License

Licensed under the terms of MIT license. See [LICENSE](LICENSE) for details.
