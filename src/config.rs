use std::net::{IpAddr, SocketAddr};

pub struct Config {
    pub socks5_server: SocketAddr,
    pub host: IpAddr,
    pub port: usize,
}
