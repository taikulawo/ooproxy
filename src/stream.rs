use std::{cell::RefCell, future::Future, io::{self}, pin::Pin, task::{Context, Poll}};

use tokio::{io::{AsyncRead, ReadBuf}, net::TcpStream};

macro_rules! try_poll {
    ($expr:expr) => {
        match $expr {
            // try_poll 表达式返回 usize
            // 如果下面这行改为 => Poll::Pending,则表达式还有 Pending 类型，和 usize 歧义
            // 所以 return 意思是直接返回调用函数，不会赋值
            // 所以 try_poll! 的类型才会是 usize
            Poll::Pending => return Poll::Pending,
            Poll::Ready(Err(err)) => return Poll::Ready(Err(err)),
            // 用下面这行会报错，试试？
            // Poll::Ready(Ok(v)) => return v,
            Poll::Ready(Ok(v)) => v,
        }
    };
}
const SHARED_BUF_SIZE: usize = 1024 * 64;
const PRIVATE_BUF_SIZE: usize = 1024 * 8;
thread_local! {
    static SHARED_BUFFER: RefCell<[u8; SHARED_BUF_SIZE]> = RefCell::new([0u8; SHARED_BUF_SIZE]);
}
pub struct StreamWithBuffer {
    pub stream: TcpStream,
    buf: Option<Box<[u8]>>,
    pos: usize,
    cap: usize,
    pub read_eof: bool,
    pub all_done: bool,
}

impl StreamWithBuffer {
    pub fn new(stream: TcpStream) -> Self {
        StreamWithBuffer {
            stream,
            buf: None,
            pos: 0,
            cap: 0,
            read_eof: false,
            all_done: false
        }
    }
    pub fn is_empty(&self) -> bool {
        self.pos == self.cap
    }
    // Read from self.stream, put the data into buffer
    pub fn poll_read_to_buffer(&mut self, cx: &mut Context) -> Poll<io::Result<usize>> {
        let stream = Pin::new(&mut self.stream);

        let n = try_poll!(if let Some(ref mut buf) = self.buf {
            let mut buf = ReadBuf::new(buf);
            stream
                .poll_read(cx, &mut buf)
                .map_ok(|_|buf.filled().len())
        }else {
            SHARED_BUFFER.with(|buf| {
                let shared_buf = &mut buf.borrow_mut()[..];
                let mut buf = ReadBuf::new(shared_buf);
                stream
                    .poll_read(cx, &mut buf)
                    .map_ok(|_| buf.filled().len())
            })
        });
        
        if n == 0 {
            self.read_eof = true;
        }else {
            self.pos = 0;
            self.cap = n;
        }
        Poll::Ready(Ok(n))
    }
}
pub struct BiPipe {
    left: StreamWithBuffer,
    right: StreamWithBuffer
}

pub fn pipe(left: StreamWithBuffer, right: StreamWithBuffer) -> BiPipe {
    BiPipe {
        left,
        right,
    }
}

impl BiPipe {
    pub fn poll_one_side(&mut self, ctx: Context) {

    }
}

// impl Future for BiPipe {
//     fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        
//     }
// }