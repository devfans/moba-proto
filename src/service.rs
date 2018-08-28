
use serde_json::Value;
use std::net::SocketAddr;
use tokio::{net, timer};

use futures::Sink;
use std::io;
use tokio_codec;
use tokio;

use model::*;
use futures::{future, Stream, Future};
use futures::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};


#[allow(dead_code)]
pub struct Client {
    stream: Mutex<mpsc::UnboundedSender<Message>>,
    active: AtomicBool,
    client_id: String,
    status: Status,
}

impl Client {
    #[allow(dead_code)]
    pub fn send(&self, msg: Message) -> bool {
        if !self.active.load(Ordering::Acquire) {
            return false;
        }

        if match self.stream.lock().unwrap().start_send(msg) {
            Ok(sink) => sink.is_ready(),
            Err(_) => false,
        } { true } else {
            self.active.store(false, Ordering::Release);
            false
        }
    } 
}

pub struct Server {
    clients: Mutex<(Vec<Arc<Client>>, u64)>,
}

impl Server {
    #[allow(dead_code)]
    pub fn new() -> Arc<Server> {
        let server = Arc::new(Server {
            clients: Mutex::new((Vec::new(), 0)),
        });
        let s = server.clone();
        tokio::spawn(timer::Interval::new(Instant::now(), Duration::from_secs(1))
            .for_each(move|_| {
                let clients = s.clients.lock().unwrap().0.clone();
                let mut clean_up = false;
                for client in clients {
                    if !client.send(Message::BattleReady { raw: vec![0] }) {
                        clean_up = true;
                    };
                }
                if clean_up {
                    println!("dropping disconnected client");
                }
                println!("Ticking");
                future::result(Ok(()))
            }).map_err(|e| panic!("interval errored; err={:?}", e))
            .then(|_| {
                println!("Broadcasting ended");
                future::result(Ok(()))
            }));
        server
    }
}

#[derive(Clone)]
pub struct Service {
    conf: Value,
    pub addr: SocketAddr,
}

impl Service {
    pub fn new(conf: Value) -> Self {
        let addr = conf["server"].as_str().unwrap().parse().expect("Failed to parse listening address");
        Service {
            conf: conf.clone(),
            addr
        }
    }
    #[allow(dead_code)]
    pub fn handle(&self, server: Arc<Server>, sock: net::TcpStream) {
        sock.set_nodelay(true).unwrap();
        let (tx, rx) = tokio_codec::Framed::new(sock, MessageFramer::new()).split();
        let (sink, send) = mpsc::unbounded();
        let stream = send.map_err(|_| -> io::Error {
            panic!("mpsc streams cant generate errors!");
        });
        tokio::spawn(tx.send_all(stream).then(|_| {
            println!("Disconnected on send side, will reconnect...");
            future::result(Ok(()))
        }));

        {
            let client = Arc::new(Client {
                stream: Mutex::new(sink),
                active: AtomicBool::new(true),
                client_id: String::new(),
                status: Status::Connected,
            });
            let mut clients = server.clients.lock().unwrap();
            clients.1 += 1;
            clients.0.push(client);
        }

        tokio::spawn(rx.for_each(move |msg| {
            println!("Received: {:?}", msg);
            Ok(())
        }).map_err(|_| {})
        );
    }

    #[allow(dead_code)]
    pub fn connect(self) {
        tokio::spawn(net::TcpStream::connect(&self.addr)
        .then(move |res| -> future::FutureResult<(), ()> {
            match res {
                Ok(stream) => {
                    stream.set_nodelay(true).unwrap();
                    let (tx, rx) = tokio_codec::Framed::new(stream, MessageFramer::new()).split();
                    let client = Arc::new(self);
                    let client_loop = client.clone();
                    let client_bk = client.clone();
                    let (sink, send) = mpsc::unbounded();
                    let stream = send.map_err(|_| -> io::Error {
					    panic!("mpsc streams cant generate errors!");
					});
					tokio::spawn(tx.send_all(stream).then(|_| {
						println!("Disconnected on send side, will reconnect...");
						future::result(Ok(()))
					}));

                    tokio::spawn(future::result(client_loop.start_client(sink)).map_err(|_| {}));

                    tokio::spawn(rx.for_each(move |msg| {
                        future::result(client.handle_server_message(msg))
                    })
                    .then(move |_| {
                        println!("Reconnecting...");
                        Arc::try_unwrap(client_bk).ok().unwrap().connect();
                        future::result(Ok(()))
                    }));
                },
                Err(_) => {
                    self.connect();
                },
            };
            future::result(Ok(()))
        }));
    }

    #[allow(dead_code)]
    pub fn handle_server_message(&self, msg: Message) -> Result<(), io::Error> {
        println!("RX: {:?}", msg);
        Ok(())
    }

    #[allow(dead_code)]
    pub fn start_client(&self, mut tx: mpsc::UnboundedSender<Message>) -> Result<(), io::Error> {
        match tx.start_send(Message::BattleReady { raw: vec![3, 4]}) {
            Ok(_) => {},
            Err(err) => { println!("Failed to send: {:?}", err); },
        }
        Ok(())
    }
}




