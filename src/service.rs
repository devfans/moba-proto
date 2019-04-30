
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
    players: Mutex<Vec<u8>>,
    bus: Mutex<mpsc::UnboundedSender<Message>>,
    cache: Mutex<Vec<Message>>,
    avail_id: AtomicUsize,
    frame: AtomicUsize,
}

impl Server {
    #[allow(dead_code)]
    pub fn get_frame(&self, battle: u8) -> Message {
        // TODO: Compose data frames here
        let mut game_actions = Vec::new();
        let mut cache = self.cache.lock().unwrap();
        let frame = self.frame.load(Ordering::Acquire);
        self.frame.store(frame + 1 as usize, Ordering::Release);
        for input in cache.iter() {
            match input {
                Message::DataInput { actions, .. } => { game_actions.extend_from_slice(actions); },
                _ => {},
            }
        }
        cache.clear();
        Message::DataFrame { battle: battle, frame: frame, actions: game_actions }
    }

    #[allow(dead_code)]
    pub fn new(service: Arc<Service>) -> Arc<Server> {
        let (tx, rx) = mpsc::unbounded();
        let server = Arc::new(Server {
            clients: Mutex::new((Vec::new(), 0)),
            players: Mutex::new(Vec::new()),
            bus: Mutex::new(tx),
            cache: Mutex::new(Vec::new()),
            avail_id: AtomicUsize::new(1),
            frame: AtomicUsize::new(1),
        });
        let server_bus = server.clone();
        tokio::spawn(rx.for_each(move |msg| {
            println!("Tx-DataInput, {:?}", msg);
            {
                let mut cache = server_bus.cache.lock().unwrap();
                cache.push(msg);
            }
            Ok(())
        }));
        let s = server.clone();
        let s_load = s.clone();
        let s_start = s.clone();
        let s_main = s.clone();
        let service_load = service.clone();
        let service_start = service.clone();
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
                                println!("Duplicate player ID: {}", player);
                                continue;
                            }
                            ready = ready + 1;
                            client.status.store(Status::Wait as usize, Ordering::Release);
                            println!("Player {} init", player);
                            wait_players.push(player);
                            let _ = client.send(Message::BattleInit { battle: service.battle, player: player as u8 });
                        },
                        Status::Wait => {
                            ready = ready + 1;
                            wait_players.push(player);
                        },
                        _ => {},
                    }
                }
                if ready >= service.players {
                    println!("{} players are prepared for the battle {}, will start loading", ready, service.battle);
                    clients_drop.retain(|client| client.status.load(Ordering::Acquire) == Status::Wait as usize);
                    let mut players = s.players.lock().unwrap();
                    for client in clients_drop {
                        let player = client.player.load(Ordering::Acquire);
                        players.push(player as u8);
                    }
                    Err(())
                } else {
                    println!("{} players are prepared for the battle", ready);
                    Ok(())
                }
            })
            .then(|_| {
                println!("Battle loading");
                timer::Interval::new(Instant::now(), Duration::from_secs(1))
                .map_err(|_| {})
                .for_each(move|_| {
                    let mut ready = 0;
                    let mut wait_players = Vec::new();
                    let clients = s_load.clients.lock().unwrap().0.clone();
                    let players = s_load.players.lock().unwrap();
                    for client in clients {
                        let status: Status = client.status.load(Ordering::Acquire).into();
                        let player = client.player.load(Ordering::Acquire);
                        match status {
                            Status::Wait => {
                                let _ = client.send(Message::BattleMeta { battle: service_load.battle, player: player as u8, raw: players.clone() });
                            },
                            Status::Ready => {
                                if !wait_players.contains(&player) {
                                    wait_players.push(player);
                                    println!("Player {} is ready", player);
                                }
                                ready = ready + 1;
                            },
                            _ => {},
                        }
                    }
                    if ready >= service_load.players {
                        println!("{} players are ready for the battle {}, will pre-start", ready, service_load.battle);
                        Err(())
                    } else {
                        println!("{} players are ready", ready);
                        Ok(())
                    }
                })
            }).map_err(|_| { println!("Battle loading done") } )
            .then(|_| {
                println!("Battle pre-starting");
                timer::Interval::new(Instant::now(), Duration::from_secs(1))
                .map_err(|_| {})
                .for_each(move|_| {
                    let mut ready = 0;
                    let mut wait_players = Vec::new();
                    let clients = s_start.clients.lock().unwrap().0.clone();
                    for client in clients {
                        let status: Status = client.status.load(Ordering::Acquire).into();
                        let player = client.player.load(Ordering::Acquire);
                        match status {
                            Status::Ready => {
                                let _ = client.send(Message::BattleStart { battle: service_start.battle });
                            },
                            Status::Start => {
                                if !wait_players.contains(&player) {
                                    wait_players.push(player);
                                    println!("Player {} is starting", player);
                                }
                                ready = ready + 1;
                            },
                            _ => {},
                        }
                    }
                    if ready >= service_start.players {
                        println!("{} players are starting the battle {}, battle will begin", ready, service_start.battle);
                        Err(())
                    } else {
                        println!("{} players are starting", ready);
                        Ok(())
                    }
                })
            }).map_err(|_| { println!("Battle starting now") } )
            .then(|_| {
                println!("Battle begin");
                timer::Interval::new(Instant::now(), Duration::from_millis(60))
                .map_err(|_| {})
                .for_each(move|_| {
                    let clients = s_main.clients.lock().unwrap().0.clone();
                    let mut clean_up = false;
                    let frame = s_main.get_frame(service_main.battle);
                    for client in clients {
                        if !client.send(frame.clone()) {
                            clean_up = true;
                        };
                    }
                    if clean_up {
                        println!("dropping disconnected client");
                    }
                    println!("Ticking");
                    Ok(())
                })
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
            macro_rules! send {
                ($msg : expr) => {
                    {
                        let mut tx = server.bus.lock().unwrap();
                        let success = match tx.start_send($msg) {
                            Ok(sink) => sink.is_ready(),
                            Err(err) => { 
                                println!("Failed to send: {:?}", err);
                                false
                            },
                        };
                        if !success {
                            panic!("Potential bus busy detected");
                        }
                    }
                }
            }


            println!("Received: {:?}", msg);
            let mut send_msg = false;
            match msg {
                Message::BattleInit { battle: _, player: _ } => {
                    // println!("goes here");
                    let id = server.avail_id.load(Ordering::Acquire);
                    server.avail_id.store(id + 1 as usize, Ordering::Release);
                    client.player.store(id as usize, Ordering::Release);
                    client.status.store(Status::Init as usize, Ordering::Release);
                },
                Message::BattleMeta { battle: _, player: _, raw: _ } => {
                    client.status.store(Status::Ready as usize, Ordering::Release)
                },
                Message::BattleStart { battle: _ } => {
                    client.status.store(Status::Start as usize, Ordering::Release)
                },
                Message::DataInput { .. } => {
                    send_msg = client.status.load(Ordering::Acquire) == Status::Start as usize;
                }
                _ => { println!("Unknown message!"); },
            }
            if send_msg { send!(msg); }
            future::result(Ok(()))
        }).map_err(|err| { println!("Client disconnected{:?}!", err);})
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
                    let sink = Arc::new(Mutex::new(sink));
                    let sink_clone = sink.clone();
                    let stream = send.map_err(|_| -> io::Error {
					    panic!("mpsc streams cant generate errors!");
					});
					tokio::spawn(tx.send_all(stream).then(|_| {
						println!("Disconnected on send side, will reconnect...");
						future::result(Ok(()))
					}));


                    tokio::spawn(rx.for_each(move |msg| {
                        client.handle_server_message(msg, sink.clone());
                        Ok(())
                    })
                    .then(move |_| {
                        println!("Reconnecting...");
                        Arc::try_unwrap(client_bk).ok().unwrap().connect();
                        future::result(Ok(()))
                    }));

                    client_loop.start_client(sink_clone);
                },
                Err(_) => {
                    self.connect();
                },
            };
            future::result(Ok(()))
        }));
    }

    #[allow(dead_code)]
    pub fn handle_server_message(&self, msg: Message, tx: Arc<Mutex<mpsc::UnboundedSender<Message>>>) {
        macro_rules! send {
            ($msg : expr) => {
                {
                    let mut tx = tx.lock().unwrap();
                    match tx.start_send($msg) {
                        Ok(sink) => { if !sink.is_ready() { return; } },
                        Err(err) => { println!("Failed to send: {:?}", err); return; },
                    }
                }
            }
        }

        println!("RX: {:?}", msg);
        match msg {
            Message::BattleMeta { battle: _, player: _, raw: _ } => {
                send!(msg);
            },
            Message::BattleStart { battle: _  } => {
                send!(msg);
            },
            _ => {},
        }
    }

    #[allow(dead_code)]
    pub fn start_client(&self, tx: Arc<Mutex<mpsc::UnboundedSender<Message>>>) {
        macro_rules! send {
            ($msg : expr) => {
                {
                    let mut tx = tx.lock().unwrap();
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
        tokio::spawn(timer::Interval::new(Instant::now(), Duration::from_secs(1))
            .for_each(move |_| {
                let mut tx = tx.lock().unwrap();
                let _ = tx.start_send(Message::DataInput{ battle, player, actions: Vec::new() });
                future::result(Ok(()))
            })
            .map_err(|_| {})
        );
    }
}




