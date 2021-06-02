use actix_web::{dev::Server, get, post, rt, web, App, HttpResponse, HttpServer, Responder};
use anyhow::{anyhow, Result};
use parking_lot::Mutex;
use std::{sync::Arc, thread, time::Duration};
use tokio::sync::oneshot;

#[get("/health")]
async fn health() -> &'static str {
    dbg!(&"HELLO");
    "Hello world!"
}

#[post("/stop")]
async fn stop(control_panel: web::Data<Arc<ControlPanel>>) -> impl Responder {
    let stop_receiver = control_panel.get_ref().clone().stop();
    let _outcome = stop_receiver.await;
    dbg!(&"STOP AWAITED");

    HttpResponse::NoContent().finish()
}

struct ControlPanel {
    address: String,
    server: Arc<Mutex<Option<Server>>>,
}

impl ControlPanel {
    pub(crate) fn new(address: &str) -> Arc<Self> {
        Arc::new(Self {
            address: address.to_owned(),
            server: Arc::new(Mutex::new(None)),
        })
    }

    /// Start Actix Server in new thread
    pub(crate) fn start(self: Arc<Self>) {
        thread::spawn(move || {
            if let Err(error) = self.start_server() {
                dbg!("Unable start rest api server: {}", error.to_string());
            }
        });
    }

    /// Stop Actix Server if it is working.
    /// Returned receiver will take a message when shutdown are completed
    fn stop(self: Arc<Self>) -> tokio::sync::oneshot::Receiver<Result<()>> {
        let (tx, rx) = oneshot::channel();

        let cloned_self = self.clone();
        let runtime_handler = tokio::runtime::Handle::current();
        thread::spawn(move || {
            let maybe_server = cloned_self.server.lock();

            match &(*maybe_server) {
                Some(server) => runtime_handler.block_on(async {
                    server.stop(false).await;

                    dbg!(&"SERVER STOPPED");
                    let _ = tx.send(Ok(()));
                }),
                None => {
                    let error_message =
                        "Unable to stop ControlPanel because server is not runnning";
                    dbg!("{}", error_message);
                    let _ = tx.send(Err(anyhow!(error_message)));
                }
            }
        });

        rx
    }

    fn start_server(self: Arc<Self>) -> std::io::Result<()> {
        let address = self.address.clone();

        let cloned_self = self.clone();
        let system = Arc::new(rt::System::new());
        let server = HttpServer::new(move || {
            App::new()
                .data(cloned_self.clone())
                .service(health)
                .service(stop)
        })
        .bind(&address)?
        .shutdown_timeout(1)
        .workers(1);

        system.block_on(async {
            *self.server.lock() = Some(server.run());
        });

        Ok(())
    }
}

#[actix_web::main]
async fn main() {
    ControlPanel::new("127.0.0.1:8080").start();

    tokio::time::sleep(Duration::from_secs(20)).await;
}
