use std::{borrow::Cow, io, net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6}, time::Duration, vec};

use bytes::BytesMut;
use log::{debug, info};
use tokio::{io::{AsyncReadExt, AsyncWriteExt}, net::TcpStream, time::timeout};

use crate::{linux::{get_original_address_v4, get_original_address_v6}, tls};

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

impl From<SocketAddr> for Destination {
    fn from(addr: SocketAddr) -> Self {
        Destination {
            host: Address::Ip(addr.ip()),
            port: addr.port()
        }
    }
}

impl<'a> From<(&'a str, u16)> for Destination {
    fn from(addr: (&'a str, u16)) -> Self {
        let host = String::from(addr.0).into_boxed_str();
        Destination {
            host: Address::Domain(host),
            port: addr.1,
        }
    }
}

impl From<(Address, u16)> for Destination {
    fn from(addr_port: (Address, u16)) -> Self {
        Destination {
            host: addr_port.0,
            port: addr_port.1,
        }
    }
}


pub struct Client {
    left: TcpStream,
    src: SocketAddr,
    pub dest: Destination,
    from_port: u16,
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
    pub async fn from_socket(mut peer_left: TcpStream) -> io::Result<Self> {
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
            let addr: Address = match buf[3] {
                0x01 => {
                    // ipv4
                    let mut buf = [0u8; 4];
                    peer_left.read_exact(&mut buf).await;
                    buf.into()
                }
                0x03 => {
                    // domain
                    // socks v5 domain first byte 是 domain 长度
                    let domain_len = peer_left.read_u8().await? as usize;
                    buf.resize(domain_len, 0);
                    let raw_ipv4 = peer_left.read_exact(&mut buf).await?;
                    let domain = String::from_utf8(buf).map_err(|_| {
                        io::Error::new(io::ErrorKind::InvalidInput, "Socksv5: invalid domain name")
                    })?;
                    domain.into()
                }
                0x04 => {
                    // ipv6
                    let mut buf = [0u8; 16];
                    peer_left.read_exact(&mut buf).await?;
                    buf.into()
                }
                _ => return error_invalid_input("Socksv5: unknown address type")
            };
            let port = peer_left.read_u16().await?;
            peer_left.write_all(&[5, 0, 0, 1, 0, 0, 0, 0, 0, 0]).await?;
            (addr, port).into()
        };
        Ok(Client {
            // 上面的 dest 类型直到这里 dest 赋值给 Destination 类型的字段成员
            // dest 的类型才真正被确认，之前的 into 一直推导出 unknown
            dest: dest,
            from_port: src_port,
            left: peer_left,
            src: left_src,
        })
    }
    
}

impl Client {
    // REDIRECT 情况下不会有 socks 的握手流程
    // 起手流量是 TLS client hello
    // 需要我们从 TLS 嗅探出 domain name
    // 注意： 这里是 (self), 所以这函数会 contume 掉 Self
    pub async fn retrive_dest(self) -> io::Result<Client> {
        let Client {
            mut left,
            src,
            dest,
            from_port,
        } = self;
        let wait = Duration::from_millis(500);
        let mut buf = BytesMut::with_capacity(2048);
        buf.resize(buf.capacity(), 0);
        if let Ok(len) = timeout(wait, left.read(&mut buf)).await? {
            buf.truncate(len);
            match tls::parse_client_hello(&buf) {
                Err(err) => info!("fail to parse hello: {}", err),
                Ok(parser) => {
                    
                }
            }
        }
        Ok(Client{
            dest: dest,
            from_port,
            left,
            src
        })
    }
}