use std::time::Duration;

use axum::routing::get;
use axum::Router;
use employees::{Shutdown, Worker};

/* ---------- */

pub(crate) struct HttpServer;

impl Worker for HttpServer {
    fn run(&mut self, shutdown: Shutdown) {
        let runtime =
            tokio::runtime::Runtime::new().expect("failed to build the http server runtime");

        runtime.block_on(async {
            let app = Router::new().route("/", get(|| async { "Bonjour, Monde !" }));

            axum::Server::bind(
                &"0.0.0.0:23456"
                    .parse()
                    .expect("invalid http server endpoint"),
            )
            .serve(app.into_make_service())
            .with_graceful_shutdown(async {
                while shutdown.is_running() {
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
            })
            .await
            .expect("http server returned with an error")
        });
    }
}
