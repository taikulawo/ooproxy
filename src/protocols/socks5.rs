use std::io::{self, ErrorKind};
use std::net::IpAddr;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use crate::client::{Address, Destination};

macro_rules! err {
    ($msg: expr) => {
        return Err(io::Error::new(ErrorKind::Other, $msg));
    };
}
pub async fn handshake<T>(
    remote: &mut TcpStream,
    dest: &Destination,
    data: Option<T>,
) -> io::Result<()>
where
    T: AsRef<[u8]>,
{
    let Destination { ref host, ref port } = dest;
    // 终于到我最熟悉的socks5协议了
    // 下面开始socks5握手
    // https://tools.ietf.org/html/rfc1928#section-3
    do_handshake(remote, dest, data).await?;
    Ok(())
}
async fn do_handshake<T>(
    remote: &mut TcpStream,
    dest: &Destination,
    data: Option<T>,
) -> io::Result<()>
where
    T: AsRef<[u8]>,
{
    // +----+----------+----------+
    // |VER | NMETHODS | METHODS  |
    // +----+----------+----------+
    // | 1  |    1     | 1 to 255 |
    // +----+----------+----------+
    // we don't support user auth;
    remote.write_all(&[0x05, 0x01, 0x00]).await?;
    // 只读 2 字节
    let mut buf = vec![0; 2];
    remote.read_exact(&mut buf).await?;
    match buf[..] {
        [0x05, 0x00] => (),
        _ => err!(""),
    }
    let request = build_request(dest)?;
    remote.write_all(&request).await?;
    let mut buf = vec![0, 10];
    // 0x05,0x00,0x00,0x01,0x00,0x00,0x00,0x00,0x00,0x00
    remote.read_exact(&mut buf).await?;
    if buf[..2] != [0x05, 0x00] {
        err!("unexpected reply from server");
    }
    // handshake has ended
    // write out all data from client
    // pipe started
    if let Some(data) = data {
        remote.write_all(data.as_ref()).await?;
    }
    Ok(())
}

fn build_request(dest: &Destination) -> io::Result<Vec<u8>> {
    // https://tools.ietf.org/html/rfc1928#section-4
    let mut buf = vec![];
    buf.extend(&[0x05, 0x01, 0x00]);
    match dest.host {
        Address::Ip(ip) => match ip {
            IpAddr::V4(i) => {
                //    the address is a version-4 IP address, with a length of 4 octets
                buf.extend_from_slice(&i.octets())
            }
            IpAddr::V6(i) => {
                buf.push(0x04);
                //   the address is a version-6 IP address, with a length of 16 octets.
                buf.extend(&i.octets());
            }
        },
        Address::Domain(ref name) => {
            let len = name.len();
            buf.push(0x03);
            buf.push(len as u8);
            buf.extend(name.as_bytes());
        }
    }
    // 端口两字节
    buf.push((dest.port >> 8) as u8);
    buf.push(dest.port as u8);
    Ok(buf)
}
