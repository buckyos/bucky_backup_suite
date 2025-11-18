use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::fs::File;
use tokio::io::{AsyncRead, AsyncSeekExt, ReadBuf};

/// 一个只允许读取文件中指定数据块的AsyncRead读取器
pub struct RangeReader {
    reader: Pin<Box<dyn AsyncRead + Unpin + Send>>,
    size: u64,
    read: u64,
}

impl RangeReader {
    /// 创建一个新的 ChunkedFileReader
    ///
    /// # 参数
    /// - `file`: 已打开的文件（需要具有读取权限）
    /// - `start`: 要读取的数据块在文件中的起始位置（字节偏移量）
    /// - `size`: 要读取的数据块大小（字节数）
    ///
    /// # 注意
    /// 创建此读取器不会立即检查文件是否足够大。
    pub fn new(reader: Pin<Box<dyn AsyncRead + Unpin + Send>>, size: u64) -> Self {
        Self {
            reader,
            size,
            read: 0,
        }
    }

    /// 异步地打开文件并创建 RangeReader
    ///
    /// # 参数
    /// - `path`: 文件路径
    /// - `start`: 要读取的数据块在文件中的起始位置
    /// - `size`: 要读取的数据块大小
    pub async fn from_file<P: AsRef<std::path::Path>>(
        path: P,
        start: u64,
        size: u64,
    ) -> Result<Self, std::io::Error> {
        let mut file = File::open(path).await?;
        file.seek(std::io::SeekFrom::Start(start)).await?;
        Ok(Self::new(Box::pin(file), size))
    }
}

impl AsyncRead for RangeReader {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        // 检查是否已经读取了完整的数据块
        if self.read >= self.size {
            return Poll::Ready(Ok(())); // 返回成功，但读取0字节（EOF）
        }

        // 计算本次读取的最大允许字节数
        let remaining_in_chunk = self.size - self.read;
        let max_to_read = buf.remaining().min(remaining_in_chunk as usize);

        if max_to_read == 0 {
            return Poll::Ready(Ok(()));
        }

        // 限制本次读取的缓冲区大小
        let mut limited_buf = ReadBuf::new(&mut buf.initialize_unfilled()[..max_to_read]);

        // 委托给内部的 file.poll_read，但使用限制后的缓冲区
        let reader = self.get_mut();
        match reader.reader.as_mut().poll_read(cx, &mut limited_buf) {
            Poll::Ready(Ok(())) => {
                let filled = limited_buf.filled().len();
                // 更新缓冲区的填充状态
                buf.advance(filled);
                // 更新已读取字节数
                reader.read += filled as u64;

                Poll::Ready(Ok(()))
            }
            Poll::Ready(Err(e)) => Poll::Ready(Err(e)),
            Poll::Pending => Poll::Pending,
        }
    }
}
