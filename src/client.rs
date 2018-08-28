
extern crate serde_json;
extern crate futures;
extern crate tokio;
extern crate tokio_codec;
extern crate bytes;
extern crate tokio_io;

use serde_json::Value;
mod service;
use service::*;
mod model;

use std::fs;
use std::env;
use futures::future;

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
    let service_ref = service.clone();
    let listen_addr = service.addr;
    println!("Client is connecting to {}", listen_addr);
    tokio::run(future::lazy(move || -> Result<(), ()> {
        service_ref.connect();
        Ok(())
    }));
}
