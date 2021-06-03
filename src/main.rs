use actix_web::{dev::Server, get, post, rt, web, App, HttpResponse, HttpServer, Responder};
use anyhow::{anyhow, Result};
use futures::executor;
use parking_lot::Mutex;
use std::{sync::mpsc, sync::Arc, thread, time::Duration};
use tokio::sync::oneshot;

#[get("/health")]
async fn health() -> &'static str {
    dbg!(&"HELLO");
    "Hello world!"
}

#[post("/stop")]
async fn stop(server_stopper: web::Data<mpsc::Sender<()>>) -> impl Responder {
    dbg!(&"STOP ENDPOINT");
    server_stopper.send(()).expect("Problem in send");

    HttpResponse::NoContent().finish()
}

struct ControlPanel {
    address: String,
    tx: Arc<Mutex<Option<mpsc::Sender<()>>>>,
}

impl ControlPanel {
    pub(crate) fn new(address: &str) -> Arc<Self> {
        Arc::new(Self {
            address: address.to_owned(),
            tx: Arc::new(Mutex::new(None)),
        })
    }

    /// Start Actix Server in new thread
    pub(crate) fn start(self: Arc<Self>) {
        let (tx, rx) = mpsc::channel::<()>();
        *self.tx.lock() = Some(tx.clone());
        let server =
            HttpServer::new(move || App::new().data(tx.clone()).service(health).service(stop))
                .bind(&self.address)
                .expect("Unable to bind")
                .shutdown_timeout(1)
                .workers(1)
                .run();

        let cloned_server = server.clone();
        thread::spawn(move || {
            rx.recv().unwrap();

            executor::block_on(cloned_server.stop(false));
        });

        thread::spawn(move || {
            if let Err(error) = self.start_server(server) {
                dbg!("Unable start rest api server: {}", error.to_string());
            }
        });
    }

    /// Stop Actix Server if it is working.
    /// Returned receiver will take a message when shutdown are completed
    fn stop(self: Arc<Self>) {
        self.tx
            .lock()
            .clone()
            .expect("There are no tx in structure")
            .send(())
            .expect("Unable_to_send_message");
    }

    fn start_server(self: Arc<Self>, server: Server) -> std::io::Result<()> {
        let system = Arc::new(rt::System::new());

        system.block_on(server);

        Ok(())
    }
}

#[actix_web::main]
async fn main() {
    let control_panel = ControlPanel::new("127.0.0.1:8081");
    control_panel.clone().start();

    tokio::time::sleep(Duration::from_secs(20)).await;
}
