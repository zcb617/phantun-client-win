use clap::{crate_version, Arg, ArgAction, Command};
use fake_tcp::packet::MAX_PACKET_LEN;
use fake_tcp::{Socket, Stack};
use log::{debug, error, info};
use phantun::utils::{new_udp_reuseport, udp_recv_pktinfo};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
#[cfg(target_os = "linux")]
use std::net::{SocketAddrV4, SocketAddrV6};
use std::sync::Arc;
use tokio::sync::{Notify, RwLock};
use tokio::time;
use tokio_util::sync::CancellationToken;

use phantun::UDP_TTL;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Route {
    dest: String,
    gateway: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Config {
    #[serde(default = "default_local")]
    local: String,
    #[serde(default = "default_remote")]
    remote: String,
    #[serde(default)]
    ipv4_only: bool,
    #[serde(default = "default_tun_local")]
    tun_local: String,
    #[serde(default = "default_tun_peer")]
    tun_peer: String,
    #[serde(default = "default_tun_local6")]
    tun_local6: String,
    #[serde(default = "default_tun_peer6")]
    tun_peer6: String,
    #[serde(default)]
    routes: Vec<Route>,
}

fn default_local() -> String {
    "127.0.0.1:8080".to_string()
}
fn default_remote() -> String {
    "127.0.0.1:65000".to_string()
}
fn default_tun_local() -> String {
    "192.168.200.1".to_string()
}
fn default_tun_peer() -> String {
    "192.168.200.2".to_string()
}
fn default_tun_local6() -> String {
    "fcc8::1".to_string()
}
fn default_tun_peer6() -> String {
    "fcc8::2".to_string()
}

impl Default for Config {
    fn default() -> Self {
        Self {
            local: default_local(),
            remote: default_remote(),
            ipv4_only: false,
            tun_local: default_tun_local(),
            tun_peer: default_tun_peer(),
            tun_local6: default_tun_local6(),
            tun_peer6: default_tun_peer6(),
            routes: Vec::new(),
        }
    }
}

fn load_config(path: &str) -> Config {
    match fs::read_to_string(path) {
        Ok(content) => serde_json::from_str(&content).unwrap_or_else(|e| {
            error!("Failed to parse config file {}: {}, using defaults", path, e);
            Config::default()
        }),
        Err(e) => {
            error!("Failed to read config file {}: {}, using defaults", path, e);
            Config::default()
        }
    }
}

#[tokio::main]
async fn main() -> io::Result<()> {
    pretty_env_logger::init();

    // Try to load config from current directory
    let mut config = load_config("phantun-client.json");

    let matches = Command::new("Phantun Client")
        .version(crate_version!())
        .author("Datong Sun (github.com/dndx)")
        .arg(
            Arg::new("config")
                .short('c')
                .long("config")
                .required(false)
                .value_name("PATH")
                .help("Path to config file (default: phantun-client.json in current directory)")
        )
        .arg(
            Arg::new("local")
                .short('l')
                .long("local")
                .required(false)
                .value_name("IP:PORT")
                .help("Sets the IP and port where Phantun Client listens for incoming UDP datagrams")
        )
        .arg(
            Arg::new("remote")
                .short('r')
                .long("remote")
                .required(false)
                .value_name("IP or HOST NAME:PORT")
                .help("Sets the address or host name and port where Phantun Client connects to Phantun Server")
        )
        .arg(
            Arg::new("ipv4_only")
                .long("ipv4-only")
                .short('4')
                .required(false)
                .help("Only use IPv4 address when connecting to remote")
                .action(ArgAction::SetTrue)
        )
        .arg(
            Arg::new("tun_local")
                .long("tun-local")
                .required(false)
                .value_name("IP")
                .help("Sets the Tun interface IPv4 local address (O/S's end)")
        )
        .arg(
            Arg::new("tun_peer")
                .long("tun-peer")
                .required(false)
                .value_name("IP")
                .help("Sets the Tun interface IPv4 destination (peer) address (Phantun Client's end)")
        )
        .get_matches();

    // If custom config path specified, reload
    if let Some(config_path) = matches.get_one::<String>("config") {
        config = load_config(config_path);
    }

    // Command line args override config file
    let local_str = matches.get_one::<String>("local").map(|s| s.clone()).unwrap_or(config.local);
    let remote_str = matches.get_one::<String>("remote").map(|s| s.clone()).unwrap_or(config.remote);
    let ipv4_only = matches.get_flag("ipv4_only") || config.ipv4_only;
    let tun_local_str = matches.get_one::<String>("tun_local").map(|s| s.clone()).unwrap_or(config.tun_local);
    let tun_peer_str = matches.get_one::<String>("tun_peer").map(|s| s.clone()).unwrap_or(config.tun_peer);

    let local_addr: SocketAddr = local_str.parse().expect("bad local address");

    let remote_addr = tokio::net::lookup_host(&remote_str)
        .await
        .expect("bad remote address or host")
        .find(|addr| !ipv4_only || addr.is_ipv4())
        .expect("unable to resolve remote host name");
    info!("Remote address is: {}", remote_addr);

    let tun_local: Ipv4Addr = tun_local_str.parse().expect("bad local address for Tun interface");
    let tun_peer: Ipv4Addr = tun_peer_str.parse().expect("bad peer address for Tun interface");

    let (tun_local6, tun_peer6): (Option<std::net::Ipv6Addr>, Option<std::net::Ipv6Addr>) = if ipv4_only {
        (None, None)
    } else {
        (
            Some(config.tun_local6.parse().expect("bad local address for Tun interface")),
            Some(config.tun_peer6.parse().expect("bad peer address for Tun interface")),
        )
    };

    info!("TUN local: {}, TUN peer: {}", tun_local, tun_peer);
    info!("UDP listen: {} -> remote {}", local_addr, remote_addr);

    let num_cpus = num_cpus::get();
    info!("{} cores available", num_cpus);

    #[cfg(target_os = "linux")]
    let tun_devices = {
        use fake_tcp::tun::linux::LinuxTun;
        use tokio_tun::TunBuilder;

        let tun = TunBuilder::new()
            .name("")
            .up()
            .address(tun_local)
            .destination(tun_peer)
            .queues(num_cpus)
            .build()
            .unwrap();

        if remote_addr.is_ipv6() {
            phantun::utils::assign_ipv6_address(tun[0].name(), tun_local6.unwrap(), tun_peer6.unwrap());
        }

        info!("Created TUN device {}", tun[0].name());

        tun.into_iter()
            .map(|t| Arc::new(LinuxTun::new(t)) as Arc<dyn fake_tcp::tun::TunDevice>)
            .collect()
    };

    #[cfg(target_os = "windows")]
    let tun_devices = {
        use fake_tcp::tun::windows::WinDivertTun;

        info!("Opening WinDivert handle for remote {}:{}", remote_addr.ip(), remote_addr.port());
        let tun = WinDivertTun::new(remote_addr)
            .expect("Failed to open WinDivert handle (administrator privileges required)");
        info!("WinDivert ready — no virtual adapter needed");

        vec![Arc::new(tun) as Arc<dyn fake_tcp::tun::TunDevice>]
    };

    #[cfg(target_os = "windows")]
    let tun_local_addr = {
        let probe = std::net::UdpSocket::bind("0.0.0.0:0")
            .expect("Failed to bind probe socket");
        probe.connect("8.8.8.8:53")
            .expect("Failed to connect probe socket");
        match probe.local_addr().expect("Failed to get local addr") {
            SocketAddr::V4(addr) => *addr.ip(),
            _ => panic!("No IPv4 address available"),
        }
    };
    #[cfg(target_os = "linux")]
    let tun_local_addr = tun_peer;

    info!("Fake TCP local IP: {}", tun_local_addr);

    let udp_sock = Arc::new(new_udp_reuseport(local_addr));
    #[cfg(target_os = "windows")]
    let udp_sock_workers = udp_sock.clone();
    let connections = Arc::new(RwLock::new(HashMap::<SocketAddr, Arc<Socket>>::new()));

    let mut stack = Stack::new(tun_devices, tun_local_addr, tun_peer6);

    let main_loop = tokio::spawn(async move {
        let mut buf_r = [0u8; MAX_PACKET_LEN];

        loop {
            let (size, udp_remote_addr, udp_local_addr) = udp_recv_pktinfo(&udp_sock, &mut buf_r).await?;
            if let Some(sock) = connections.read().await.get(&udp_remote_addr) {
                sock.send(&buf_r[..size]).await;
                continue;
            }

            info!("New UDP client from {}", udp_remote_addr);
            let sock = stack.connect(remote_addr).await;
            if sock.is_none() {
                error!("Unable to connect to remote {}", remote_addr);
                continue;
            }

            let sock = Arc::new(sock.unwrap());

            // send first packet
            if sock.send(&buf_r[..size]).await.is_none() {
                continue;
            }

            assert!(connections
                .write()
                .await
                .insert(udp_remote_addr, sock.clone())
                .is_none());
            debug!("inserted fake TCP socket into connection table");

            let packet_received = Arc::new(Notify::new());
            let quit = CancellationToken::new();

            for i in 0..num_cpus {
                let sock = sock.clone();
                let quit = quit.clone();
                let packet_received = packet_received.clone();
                #[cfg(target_os = "windows")]
                let udp_sock = udp_sock_workers.clone();

                tokio::spawn(async move {
                    #[cfg(target_os = "linux")]
                    let mut buf_udp = [0u8; MAX_PACKET_LEN];
                    let mut buf_tcp = [0u8; MAX_PACKET_LEN];
                    #[cfg(target_os = "windows")]
                    let udp_sock = udp_sock;
                    #[cfg(target_os = "linux")]
                    let udp_sock = {
                        let bind_addr = match (udp_remote_addr, udp_local_addr) {
                            (SocketAddr::V4(_), IpAddr::V4(udp_local_ipv4)) => {
                                SocketAddr::V4(SocketAddrV4::new(
                                    udp_local_ipv4,
                                    local_addr.port(),
                                ))
                            }
                            (SocketAddr::V6(udp_remote_addr), IpAddr::V6(udp_local_ipv6)) => {
                                SocketAddr::V6(SocketAddrV6::new(
                                    udp_local_ipv6,
                                    local_addr.port(),
                                    udp_remote_addr.flowinfo(),
                                    udp_remote_addr.scope_id(),
                                ))
                            }
                            (_, _) => {
                                panic!("unexpected family combination for udp_remote_addr={udp_remote_addr} and udp_local_addr={udp_local_addr}");
                            }
                        };
                        let s = new_udp_reuseport(bind_addr);
                        s.connect(udp_remote_addr).await.unwrap();
                        s
                    };

                    #[cfg(target_os = "windows")]
                    loop {
                        tokio::select! {
                            res = sock.recv(&mut buf_tcp) => {
                                match res {
                                    Some(size) => {
                                        if size > 0 {
                                            if let Err(e) = udp_sock.send_to(&buf_tcp[..size], udp_remote_addr).await {
                                                error!("Unable to send UDP packet to {}: {}, closing connection", e, remote_addr);
                                                quit.cancel();
                                                return;
                                            }
                                        }
                                    },
                                    None => {
                                        debug!("removed fake TCP socket from connections table");
                                        quit.cancel();
                                        return;
                                    },
                                }

                                packet_received.notify_one();
                            },
                            _ = quit.cancelled() => {
                                debug!("worker {} terminated", i);
                                return;
                            },
                        };
                    }

                    #[cfg(target_os = "linux")]
                    loop {
                        tokio::select! {
                            Ok(size) = udp_sock.recv(&mut buf_udp) => {
                                if sock.send(&buf_udp[..size]).await.is_none() {
                                    debug!("removed fake TCP socket from connections table");
                                    quit.cancel();
                                    return;
                                }

                                packet_received.notify_one();
                            },
                            res = sock.recv(&mut buf_tcp) => {
                                match res {
                                    Some(size) => {
                                        if size > 0
                                            && let Err(e) = udp_sock.send(&buf_tcp[..size]).await {
                                                error!("Unable to send UDP packet to {}: {}, closing connection", e, remote_addr);
                                                quit.cancel();
                                                return;
                                            }
                                    },
                                    None => {
                                        debug!("removed fake TCP socket from connections table");
                                        quit.cancel();
                                        return;
                                    },
                                }

                                packet_received.notify_one();
                            },
                            _ = quit.cancelled() => {
                                debug!("worker {} terminated", i);
                                return;
                            },
                        };
                    }
                });
            }

            let connections = connections.clone();
            tokio::spawn(async move {
                loop {
                    let read_timeout = time::sleep(UDP_TTL);
                    let packet_received_fut = packet_received.notified();

                    tokio::select! {
                        _ = read_timeout => {
                            info!("No traffic seen in the last {:?}, closing connection", UDP_TTL);
                            connections.write().await.remove(&udp_remote_addr);
                            debug!("removed fake TCP socket from connections table");

                            quit.cancel();
                            return;
                        },
                        _ = quit.cancelled() => {
                            connections.write().await.remove(&udp_remote_addr);
                            debug!("removed fake TCP socket from connections table");
                            return;
                        },
                        _ = packet_received_fut => {},
                    }
                }
            });
        }
    });

    tokio::join!(main_loop).0.unwrap()
}
