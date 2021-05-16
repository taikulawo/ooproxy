use std::{
    future::Future,
    io,
    pin::Pin,
    task::{Context, Poll},
};

use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
const MAX_PRIVATE_BUF_SIZE: usize = 1024;

pub struct Copy<'a, R, W> {
    reader: &'a mut R,
    writer: &'a mut W,
    buf: Box<[u8]>,
    is_eof: bool,
    // 需要开始写入的位置
    pos: usize,
    // 有效数据
    cap: usize,
}
macro_rules! ready {
    ($exp: expr) => {
        match $exp {
            Poll::Pending => return Poll::Pending,
            Poll::Ready(Ok(n)) => n,
            Poll::Ready(Err(err)) => {
                return Poll::Ready(Err(io::Error::new(
                    io::ErrorKind::Other,
                    format!("try poll failed with error {}", err),
                )))
            }
        }
    };
}
impl<R, W> Future for Copy<'_, R, W>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
{
    type Output = io::Result<usize>;
    fn poll(mut self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.pos == self.cap && !self.is_eof {
            // 下面这有点意思
            // Pin实现了DerefMut，而DerefMut返回的是P::Target
            // 是 &mut Self 指向的结构
            // 这里 *self 就是 BiPipe<R,W> 类型
            // self是 mut self，所以 *self走 DerefMut
            // #[stable(feature = "pin", since = "1.33.0")]
            // impl<P: DerefMut<Target: Unpin>> DerefMut for Pin<P> {
            //     fn deref_mut(&mut self) -> &mut P::Target {
            //         Pin::get_mut(Pin::as_mut(self))
            //     }
            // }
            // Pin的 DerefMut实现获取 &mut Self，
            // get_mut 获取 &mut Self，所以*self获得 Self
            // 所以 *Pin<T> 实际为 *(&mut T)，即为 mut T
            let me = &mut *self;
            let mut b = ReadBuf::new(&mut me.buf);
            // 可写为0，需要读
            let n = ready!(Pin::new(&mut *me.reader)
                .poll_read(ctx, &mut b)
                .map_ok(|_| b.filled().len()));
            if n == 0 {
                self.is_eof = true;
            }else {
                self.pos = 0;
                self.cap = n;
            }
            while self.pos <= self.cap {
                let me = &mut *self;
                let n = ready!(Pin::new(&mut me.writer).poll_write(ctx, &me.buf[me.pos .. me.cap]));
                if n == 0 {
                    return Poll::Ready(Err(io::Error::new(io::ErrorKind::WriteZero, "write zero")));
                }
                me.pos += n;
            }
            if self.is_eof {
                let me = &mut *self;
                ready!(Pin::new(&mut me.writer).poll_flush(ctx));
                return Poll::Ready(Ok(self.pos));
            }
        }
        return Poll::Ready(Ok(0));
    }
}
pub fn copy_from_to<'a, R, W>(reader: &'a mut R, writer: &'a mut W) -> Copy<'a, R, W>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
{
    Copy {
        // buf可以优化为 uninitialized buffer，这样不需要用0填充。
        // 我们用 pos 和 cap 追踪就行。
        buf: Vec::with_capacity(MAX_PRIVATE_BUF_SIZE).into_boxed_slice(),
        reader,
        writer,
        is_eof: false,
        cap: 0,
        pos: 0,
    }
}
