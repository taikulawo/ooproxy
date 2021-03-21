use std::{borrow::Cow, io, net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6}, vec};

use log::debug;
use tokio::{io::{AsyncReadExt, AsyncWriteExt}, net::TcpStream};

use crate::linux::{get_original_address_v4, get_original_address_v6};

#[derive(Clone, Debug)]
pub enum Address {
    Ip(IpAddr),
    Domain(Box<str>)
}

impl From<[u8; 4]> for Address{
    fn from(buf: [u8; 4]) -> Self {
        Address::Ip(IpAddr::V4(Ipv4Addr::from(buf)))
    }
}

impl From<[u8; 16]> for Address {
    fn from(buf: [u8; 16]) -> Self {
        Address::Ip(IpAddr::V6(Ipv6Addr::from(buf)))
    }
}

impl From<String> for Address {
    fn from(s: String) -> Self {
        Address::Domain(s.into_boxed_str())
    }
}

pub struct Destination {
    pub host: Address,
    pub port: u16,
}
pub struct Client {

}

// 归一化处理，统一用 ipv6 比较
fn normalize_socket_addr(socket: &SocketAddr) -> Cow<SocketAddr> {
    match socket {
        SocketAddr::V4(sock) => {
            let addr = sock.ip().to_ipv6_mapped();
            let sock = SocketAddr::new(addr.into(), sock.port());
            Cow::Owned(sock)
        }
        _ => Cow::Borrowed(socket)
    }
}
fn error_invalid_input<T>(msg: &'static str) -> io::Result<T> {
    Err(io::Error::new(io::ErrorKind::InvalidInput, msg))
}
impl Client {
    pub async fn from_socket(peer_left: TcpStream) -> io::Result<Self> {
        let left_src = peer_left.peer_addr()?;
        let src_port = peer_left.local_addr()?.port();
        let dest = get_original_address_v4(&peer_left)
            .map(SocketAddr::V4)
            .or_else(|_| get_original_address_v6(&peer_left).map(SocketAddr::V6))
            // 如果 get_original_v4, 6都失败，说明是直连的代理端口
            // 这时 local_addr 就是 client 的 addr
            .or_else(|_| peer_left.local_addr())?;
        #[cfg(not(target_os = "linux"))]
        let dest = peer_left.local_addr()?;
        let is_nated = normalize_socket_addr(&dest) != normalize_socket_addr(&peer_left.local_addr()?);
        
        debug!("local {} dest{}", peer_left.local_addr()?, dest);
        let dest = if cfg!(target_os = "linux") && is_nated {
            dest.into()
        }else {
            let ver = peer_left.read_u8().await?;
            if ver != 0x05 {
                return error_invalid_input("Neither a NATed or SOCKSv5 connection ");
            }
            let n_methods = peer_left.read_u8().await?;
            let mut buf = vec![0u8; n_methods as usize];
            peer_left.read_exact(&mut buf).await?;
            if buf.iter().find(|&&m| m == 0).is_none() {
                return error_invalid_input("Socksv5, Only no auth supported");
            }
            peer_left.write_all(&[0x05, 0x00]).await?;
            buf.resize(4, 0);
            peer_left.read_exact(&mut buf).await?;
            if buf[0..2] != [0x05, 0x01] {
                return error_invalid_input("Socksv5, CONNECT is required");
            }
            let addr: Address
        }
        Ok(Client {

        })
    }
}