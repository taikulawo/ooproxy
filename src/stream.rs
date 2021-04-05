use std::{cell::RefCell, cmp, future::Future, io::{self}, pin::Pin, task::{Context, Poll}, time::Duration};

use log::{debug, trace};
use tokio::{io::{AsyncRead, AsyncWrite, ReadBuf}, net::TcpStream, time::{Instant, Sleep, sleep}};
use self::Side::{Left, Right};

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
const HALF_CLOSE_TIMEOUT: Duration = Duration::from_secs(60);
thread_local! {
    static SHARED_BUFFER: RefCell<[u8; SHARED_BUF_SIZE]> = RefCell::new([0u8; SHARED_BUF_SIZE]);
}
pub struct StreamWithBuffer {
    pub stream: TcpStream,
    buf: Option<Box<[u8]>>,
    // bytebuf readIndex， writeIndex
    // https://netty.io/4.1/api/io/netty/buffer/ByteBuf.html
    // 我们这个简单，只有 pos 标记从 0 -> pos 是有效区间
    // 由于没有 更详细的 index 标记读取位置
    // 所以当poll_read_to_buffer读取完一次后
    // 剩下的 data 需要移动到 private buffer
    // 清空使用的buf
    pos: usize, // writeIndex
    cap: usize, // readIndex，buf 真正capacity写死为 SHARED_BUF_SIZE 或 PRIVATE_BUF_SIZE
    pub read_eof: bool,
    pub done: bool,
}

impl StreamWithBuffer {
    pub fn new(stream: TcpStream) -> Self {
        StreamWithBuffer {
            stream,
            buf: None,
            pos: 0,
            cap: 0,
            read_eof: false,
            done: false
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

    pub fn poll_write_buffer_to(&mut self, ctx: &mut Context, writer_stream: &mut TcpStream) -> Poll<io::Result<usize>> {
        let writer = Pin::new(writer_stream);
        let result = if let Some(ref buf) = self.buf {
            writer.poll_write(ctx, &buf[self.pos .. self.cap])
        } else {
            SHARED_BUFFER.with(|cell| {
                let buf = cell.borrow_mut();
                writer.poll_write(ctx, &buf[self.pos .. self.cap])
            })
        };
        match result {
            Poll::Ready(Ok(0)) => Poll::Ready(Err(io::Error::new(io::ErrorKind::WriteZero,"write zero bytes into writer"))),
            Poll::Ready(Ok(n)) => {
                self.pos += n;
                trace!("{} bytes written", n);
                Poll::Ready(Ok(n))
            }
            Poll::Pending if self.buf.is_none() => {
                SHARED_BUFFER.with(|shared_buf| {
                    let shared_buf = shared_buf.borrow();
                    let remaining = self.cap - self.pos;
                    let mut buf = vec![0; cmp::max(PRIVATE_BUF_SIZE, remaining)];
                    buf[..remaining].copy_from_slice(&shared_buf[self.pos .. self.cap]);
                    Poll::Pending
                })
            }
            _ => result
        }
    }
}
#[derive(Debug, Clone)]
enum Side {
    Left,
    Right
}
pub struct BiPipe {
    left: StreamWithBuffer,
    right: StreamWithBuffer,
    half_close_deadline: Option<Pin<Box<Sleep>>>,
}

pub fn pipe(left: StreamWithBuffer, right: StreamWithBuffer) -> BiPipe {
    BiPipe {
        left,
        right,
        half_close_deadline: Default::default(),
    }
}

impl BiPipe {
    fn poll_one_side(&mut self, ctx: &mut Context, side: Side) -> Poll<io::Result<()>>{
        let Self {
            ref mut left,
            ref mut right,
            ..
        } = *self;
        let (reader, writer) = match side {
            Side::Left => (left, right),
            Side::Right => (right, left),
        };
        loop {
            if reader.is_empty() && !reader.read_eof {
                let n = try_poll!(reader.poll_read_to_buffer(ctx));
            }
            while !reader.is_empty() {
                try_poll!(reader.poll_write_buffer_to(ctx, &mut writer.stream));
            }
            if reader.read_eof {
                match Pin::new(&mut writer.stream).poll_shutdown(ctx) {
                    Poll::Pending => return Poll::Pending,
                    Poll::Ready(Ok(())) => (),
                    Poll::Ready(Err(err)) => debug!("fail to shutdown: {}", err)
                }
                // writer 已经关闭
                // reader 将在另一个 poll_one_side 中作为 writer 被关闭
                reader.done = true;
                return Poll::Ready(Ok(()))
            }
        }
    }
}

impl Future for BiPipe {
    type Output = io::Result<()>;
    // https://stackoverflow.com/questions/28587698/whats-the-difference-between-placing-mut-before-a-variable-name-and-after-the
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if !self.left.done {
            if let Poll::Ready(Err(err)) = self.poll_one_side( cx, Left) {
                return Poll::Ready(Err(err))
            }
        }
        if !self.right.done {
            if let Poll::Ready(Err(err)) = self.poll_one_side(cx, Right) {
                return Poll::Ready(Err(err))
            }
        }
        match(self.left.done, self.right.done) {
            (true, true) => Poll::Ready(Ok(())),
            (false,false) => Poll::Pending,
            _ => {
                match &mut self.half_close_deadline {
                    // None 是第一次进入
                    None => {
                        let mut deadline = Box::pin(sleep(HALF_CLOSE_TIMEOUT));
                        // poll 下，通过 cx 注册 poll，由于创建完deadline后直接poll
                        // 所以一定没有 elapsed
                        // 使得下面第一个 Some 被调用
                        let _ = deadline.as_mut().poll(cx);
                        self.half_close_deadline = Some(deadline);
                        Poll::Pending
                    }
                    Some(deadline) if !deadline.is_elapsed() => {
                        // 设置超时时间
                        deadline.as_mut().reset(Instant::now() + HALF_CLOSE_TIMEOUT);
                        Poll::Pending
                    }
                    Some(_) => {
                        // 到这里说明超时了
                        debug!("(BiPipe) half-close conn timeout");
                        Poll::Ready(Ok(()))
                    }
                }
            }
        }
    }
}