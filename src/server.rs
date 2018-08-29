
extern crate serde_json;
extern crate futures;
extern crate tokio;
extern crate tokio_codec;
extern crate bytes;
extern crate tokio_io;

use serde_json::Value;
use tokio::net::TcpListener;
mod service;
use service::*;
mod model;
use futures::Stream;

use std::fs;
use std::env;
use futures::{future, Future};
use std::sync::Arc;

fn main() {
    let mut conf_path = None;

    for arg in env::args().skip(1) {
        if arg.starts_with("-c") {
            conf_path = Some(arg.split_at(14).1.to_string());
        }
    }

    let conf = if let Some(path) = conf_path {
        path
    } else {
        "./config.json".to_string()
    };
    println!("Loading configuration from {}", conf);

    let conf_file = fs::File::open(conf).expect("Failed to read config file");
    let config: Value= serde_json::from_reader(conf_file).expect("Failed to parse config file");
    let service = Service::new(config);
    let service_ref = Arc::new(service.clone());
    let listen_addr = service.addr;
    let listener = TcpListener::bind(&service.addr).unwrap();
    tokio::run(future::lazy(move || -> Result<(), ()> {
        let server = Server::new(service_ref.clone());
        let serve = listener.incoming().for_each(move |socket| {
            let service = service_ref.clone();
            service.handle(server.clone(), socket);
            Ok(())
        })
        .map_err(|err| {
            println!("accept error = {:?}", err);
        });
        tokio::spawn(serve);
        println!("Server is listening on {}", listen_addr);
        Ok(())
    }));
}
