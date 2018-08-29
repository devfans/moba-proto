
use serde_json::Value;
use std::net::SocketAddr;
use tokio::{net, timer};

use std::io;
use tokio_codec;
use tokio;

use model::*;
use futures::*;
use futures::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

#[allow(dead_code)]
pub struct Client {
    stream: Mutex<mpsc::UnboundedSender<Message>>,
    active: AtomicBool,
    player: AtomicUsize,
    status: AtomicUsize,
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
    pub fn new(service: Arc<Service>) -> Arc<Server> {
        let server = Arc::new(Server {
            clients: Mutex::new((Vec::new(), 0)),
        });
        let s = server.clone();
        let s_main = s.clone();
        let service_main = service.clone();
        println!("Initializing battle: {} with {} players", service.battle, service.players);
        tokio::spawn(timer::Interval::new(Instant::now(), Duration::from_secs(1))
            .map_err(|_| {})
            .for_each(move |_| {
                let mut ready = 0;
                let mut wait_players = Vec::new();
                let clients = s.clients.lock().unwrap().0.clone();
                let mut clients_drop = clients.clone();
                for client in clients {
                    let status: Status = client.status.load(Ordering::Acquire).into();
                    let player = client.player.load(Ordering::Acquire);
                    match status {
                        Status::Init => {
                            if wait_players.contains(&player) {
                                println!("Dumplicate player ID: {}", player);
                                continue;
                            }
                            ready = ready + 1;
                            client.status.store(Status::Wait as usize, Ordering::Release);
                            println!("Player {} is ready", player);
                            wait_players.push(player);
                        },
                        Status::Wait => {
                            ready = ready + 1;
                            wait_players.push(player);
                        },
                        _ => {},
                    }
                }
                if ready >= service.players {
                    println!("{} players will start the battle {}", ready, service.battle);
                    clients_drop.retain(|client| client.status.load(Ordering::Acquire) == Status::Wait as usize);
                    Err(())
                } else {
                    println!("{} players are ready", ready);
                    Ok(())
                }
            }).map_err(|_| { println!("Battle begins") } )
            .then(|_| {
                println!("Battle Started");
                timer::Interval::new(Instant::now(), Duration::from_secs(1))
                .for_each(move|_| {
                    let clients = s_main.clients.lock().unwrap().0.clone();
                    let mut clean_up = false;
                    for client in clients {
                        if !client.send(Message::DataFrame { battle: service_main.battle }) {
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
                })})
        );
        server
    }
}

#[derive(Clone)]
pub struct Service {
    conf: Value,
    pub addr: SocketAddr,
    pub battle: u8,
    pub players: u8,
}

impl Service {
    pub fn new(conf: Value) -> Self {
        let addr = conf["server"].as_str().unwrap().parse().expect("Failed to parse listening address");
        let battle = conf["battle"].as_u64().unwrap() as u8;
        let players = conf["players"].as_u64().unwrap() as u8;
        Service {
            conf: conf.clone(),
            addr,
            battle,
            players
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

        let client = {
            let client = Arc::new(Client {
                stream: Mutex::new(sink),
                active: AtomicBool::new(true),
                player: AtomicUsize::new(0),
                status: AtomicUsize::new(Status::Connected as usize),
            });
            let mut clients = server.clients.lock().unwrap();
            clients.1 += 1;
            clients.0.push(client.clone());
            client
        };

        tokio::spawn(rx.for_each(move |msg| {
            println!("Received: {:?}", msg);
            match msg {
                Message::BattleInit { battle, player } => {
                    client.player.store(player as usize, Ordering::Release);
                    client.status.store(Status::Init as usize, Ordering::Release)
                },
                _ => { println!("Unknown message!"); },
            }
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


                    tokio::spawn(rx.for_each(move |msg| {
                        future::result(client.handle_server_message(msg))
                    })
                    .then(move |_| {
                        println!("Reconnecting...");
                        Arc::try_unwrap(client_bk).ok().unwrap().connect();
                        future::result(Ok(()))
                    }));

                    client_loop.start_client(sink);
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
    pub fn start_client(&self, mut tx: mpsc::UnboundedSender<Message>) {
        macro_rules! send {
            ($msg : expr) => {
                {
                    match tx.start_send($msg) {
                        Ok(sink) => { if !sink.is_ready() { return; } },
                        Err(err) => { println!("Failed to send: {:?}", err); return; },
                    }
                }
            }
        }

        // Send battle ready
        let player = self.conf["player"].as_u64().unwrap() as u8;
        let battle = self.battle;
        send!(Message::BattleInit{ battle, player });
    }
}




