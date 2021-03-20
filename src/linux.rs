use std::{io::{self, ErrorKind}, mem, net::{SocketAddrV4, SocketAddrV6}, os::unix::prelude::AsRawFd};

use libc::{c_void, socklen_t};
use nix::sys::socket::{getsockopt, sockopt::OriginalDst};


// SO_ORIGINAL_DST 用于 iptables 的REDIRECT
pub fn get_original_address_v4<F>(fd: &F) -> io::Result<SocketAddrV4>
where
    F: AsRawFd,
{
    let addr = getsockopt(fd.as_raw_fd(), OriginalDst).map_err(|e| match e{
        nix::Error::Sys(err) => io::Error::from(err),
        _ => io::Error::new(ErrorKind::Other, e),
    })?;
    let addr = SocketAddrV4::new(
        u32::from_be(addr.sin_addr.s_addr).into(), 
        u16::from_be(addr.sin_port));
    Ok(addr)
}

pub fn get_original_address_v6<F>(fd: &F) -> io::Result<SocketAddrV6>
where
    F: AsRawFd,
{
    // 初始化 sockaddr_in6 结构的空内存，传到 syscall，通过指针拿到数据
    // C 语言的传数据经常用指针这么干
    let mut sockaddr: libc::sockaddr_in6 = unsafe { mem::zeroed()};
    let socklen = mem::size_of::<libc::sockaddr_in6>();
    // https://stackoverflow.com/questions/50384395/why-does-casting-from-a-reference-to-a-c-void-pointer-require-a-double-cast
    // So to recap, the double cast is necessary to first coerce from a reference to a raw pointer,
    // then from a raw pointer cast to a c_void,
    // because you otherwise normally can't cast straight from a reference to a raw c_void pointer.
    let res = unsafe {
        libc::getsockopt(fd.as_raw_fd(),
        libc::SOL_IPV6,
        libc::SO_ORIGINAL_DST,
        &mut sockaddr as *mut _ as *mut c_void,
        &mut socklen as *mut _ as *mut socklen_t)
    };
    if res != 0 {
        // 出错
        // C 常用非0作为错误返回
        return Err(io::Error::new(ErrorKind::Other, "getsockopt fail"));
    }
    // https://tools.ietf.org/html/rfc2553#section-3.3
    // 看起来 IPv6 对 socket有扩展，并不和v4一样
    let addr = SocketAddrV6::new(
        sockaddr.sin6_addr.s6_addr.into(),
        u16::from_be(sockaddr.sin6_port),
        sockaddr.sin6_flowinfo,
        sockaddr.sin6_scope_id,
    );
    Ok(addr)
}