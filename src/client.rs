use std::{io, net::{SocketAddr, SocketAddrV4, SocketAddrV6}};

use tokio::net::TcpStream;

use crate::linux::{get_original_address_v4, get_original_address_v6};

pub struct Client {

}

impl Client {
    pub fn from_socket(peer_left: TcpStream) -> io::Result<Self> {
        let left_src = peer_left.peer_addr()?;
        let src_port = peer_left.local_addr()?.port();
        let dest = get_original_address_v4(&peer_left)
            .map(SocketAddr::V4)
            .or_else(|_| get_original_address_v6(&peer_left).map(SocketAddr::V6))
            // 如果 get_original_v4, 6都失败，说明是直连的代理端口
            // 这时 local_addr 就是 client 的 addr
            .or_else(|_| peer_left.local_addr())?;
        Ok(Self)
    }
}