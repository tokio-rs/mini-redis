use crate::frame::{self, Frame};

use bytes::{Buf, BytesMut};
use tokio::io::{BufStream, AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use std::io::{self, Cursor};

#[derive(Debug)]
pub(crate) struct Connection {
    stream: BufStream<TcpStream>,
    buffer: BytesMut,
}

impl Connection {
    pub(crate) fn new(socket: TcpStream) -> Connection {
        Connection {
            stream: BufStream::new(socket),
            buffer: BytesMut::with_capacity(4 * 1024),
        }
    }

    pub(crate) async fn read_frame(&mut self) -> io::Result<Option<Frame>> {
        use frame::Error::Incomplete;

        loop {
            let mut buf = Cursor::new(&self.buffer[..]);

            match Frame::check(&mut buf) {
                Ok(_) => {
                    // Get the length of the message
                    let len = buf.position() as usize;

                    // Reset the position
                    buf.set_position(0);

                    let frame = Frame::parse(&mut buf)?;

                    // Clear data from the buffer
                    self.buffer.advance(len);

                    return Ok(Some(frame));
                }
                Err(Incomplete) => {}
                Err(e) => return Err(e.into()),
            }

            if 0 == self.stream.read_buf(&mut self.buffer).await? {
                return Ok(None);
            }
        }
    }

    pub(crate) async fn write_frame(&mut self, frame: &Frame) -> io::Result<()> {
        match frame {
            Frame::Array(val) => {
                self.stream.write_u8(b'*').await?;
                self.write_decimal(val.len() as u64).await?;

                for entry in &**val {
                    self.write_value(entry).await?;
                }
            }
            _ => self.write_value(frame).await?,
        }

        self.stream.flush().await
    }

    async fn write_value(&mut self, frame: &Frame) -> io::Result<()> {
        match frame {
            Frame::Simple(val) => {
                self.stream.write_u8(b'+').await?;
                self.stream.write_all(val.as_bytes()).await?;
                self.stream.write_all(b"\r\n").await?;
            }
            Frame::Error(val) => {
                self.stream.write_u8(b'-').await?;
                self.stream.write_all(val.as_bytes()).await?;
                self.stream.write_all(b"\r\n").await?;
            }
            Frame::Integer(val) => {
                self.stream.write_u8(b':').await?;
                self.write_decimal(*val).await?;
            }
            Frame::Null => {
                self.stream.write_all(b"$-1\r\n").await?;
            }
            Frame::Bulk(val) => {
                let len = val.len();

                self.stream.write_u8(b'$').await?;
                self.write_decimal(len as u64).await?;
                self.stream.write_all(val).await?;
                self.stream.write_all(b"\r\n").await?;
            }
            Frame::Array(_val) => unreachable!(),
        }

        Ok(())
    }

    async fn write_decimal(&mut self, val: u64) -> io::Result<()> {
        use std::io::Write;

        // Convert the value to a string
        let mut buf = [0u8; 12];
        let mut buf = Cursor::new(&mut buf[..]);
        write!(&mut buf, "{}", val)?;

        let pos = buf.position() as usize;
        self.stream.write_all(&buf.get_ref()[..pos]).await?;
        self.stream.write_all(b"\r\n").await?;

        Ok(())
    }
}
