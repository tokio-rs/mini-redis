use crate::frame::{self, Frame};

use bytes::{Buf, BytesMut};
use std::io::{self, Cursor};
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufWriter};
use tokio::net::TcpStream;

/// Send and receive `Frame` values from a remote peer.
///
/// When writing networking services, it is often useful to define an
/// intermediate representation between messages and the byte stream
/// (`TcpStream`). In the case of `mini-redis`, this representation is `Frame`.
///
/// `Connection` is initialized with a `TcpStream`. The caller sends and
/// receives `Frame` values. `Connection` is responsible for reading and writing
/// these frame values to the `TcpStream`. Doing so requires buffering in both
/// directions. When data is received from the socket, it is read into an
/// internal read buffer. Then, `Connection` attempts to decode a `Frame` value
/// from this buffer. If successful, the frame is returned to the caller. If
/// more data is needed, the function waits on more data from the socket.
///
/// When sending frame values, the frame is first encoded into the write buffer.
/// THe contents of the write buffer are then written to the socket.
#[derive(Debug)]
pub(crate) struct Connection {
    // The `TcpStream`. It is decorated with a `BufWriter`, which provides write
    // level buffering. The `BufWriter` implementation provided by Tokio is
    // sufficient for our needs.
    stream: BufWriter<TcpStream>,

    // The read level buffer. Unfortunately, `BufReader` is not sufficient for
    // our case due to some limitations. These limitations should be fixed as
    // part of Tokio 0.3. Until then, read buffering is implemented by managing
    // the buffer in `Connection`.
    buffer: BytesMut,
}

impl Connection {
    /// Create a new `Connection`, backed by `socket`. Read and write buffers
    /// are initialized.
    pub(crate) fn new(socket: TcpStream) -> Connection {
        Connection {
            stream: BufWriter::new(socket),
            // Default to a 4KB read buffer. For the use case of mini redis,
            // this is fine. However, real applications will want to tune this
            // value to their specific use case. There is a high likelihood that
            // a larger read buffer will work better.
            buffer: BytesMut::with_capacity(4 * 1024),
        }
    }

    /// Read a single `Frame` value from the underlying stream.
    ///
    /// The function waits for sufficient data to be received in order to parse
    /// a frame. Any data remaining in the read buffer after the frame is parsed
    /// is kept there for the next call to `read_frame`.
    ///
    /// # Returns
    ///
    /// On success, the received frame is returned. If the `TcpStream`
    /// connection is closed at an appropriate point, `None` is returned.
    /// Otherwise, an error is returned.
    pub(crate) async fn read_frame(&mut self) -> crate::Result<Option<Frame>> {
        use frame::Error::Incomplete;

        loop {
            // The various functions will be iterating the buffer multiple
            // times. `Cursor` is used to track the "current" location in the
            // buffer. `Cursor<impl AsRef[u8]>` also implements `Buf` from the
            // `bytes` crate which provides a number of helpful utilities for
            // working with bytes.
            let mut buf = Cursor::new(&self.buffer[..]);

            // The first step is to check if enough data has been buffered to
            // parse a single frame. This step is usually much faster than doing
            // a full parse of the frame and allows us to skip allocating data
            // structures to hold the frame data unless we know the full frame
            // has been received.
            match Frame::check(&mut buf) {
                Ok(_) => {
                    // The `check` function will have advanced the cursor until
                    // the end of the frame. Since the cursor had position set
                    // to zero before `Frame::check` was called, we obtain the
                    // length of the frame by checking the cursor position.
                    let len = buf.position() as usize;

                    // Reset the position to zero before passing the cursor to
                    // `Frame::parse`.
                    buf.set_position(0);

                    // Parse the frame from the buffer. This allocates the
                    // necessary structures to represent the frame and returns
                    // the frame value.
                    //
                    // If encoded frame representation is invalid, an error is
                    // returned. This should terminate the **current**
                    // connection but should not impact any other connected
                    // client.
                    let frame = Frame::parse(&mut buf)?;

                    // Discard the parsed data from the read buffer.
                    //
                    // `advance` is called on the read buffer which discards the
                    // data up to `len`. The details of how this works is left
                    // to `BytesMut`. This is often done by moving an internal
                    // cursor, but it may be done by reallocataing and copying
                    // data.
                    self.buffer.advance(len);

                    // Return the parsed frame to the caller.
                    return Ok(Some(frame));
                }
                // There is not enough data present in the read buffer to parse
                // a single frame. We must wait for more data to be received
                // from the socket. Reading from the socket will be done in the
                // statement after this `match`.
                //
                // We do not want to return `Err` from here as this "error" is
                // an expected runtime condition.
                Err(Incomplete) => {}
                // An error was encountered while parsing the frame. The
                // connection is now in an invalid state. Returning `Err` from
                // here will result in the connection being closed.
                Err(e) => return Err(e.into()),
            }

            // There is not enough buffered data to read a frame. Attempt to
            // read more data from the socket.
            //
            // On success, the number of bytes is returned. `0` indicates "end
            // of stream".
            if 0 == self.stream.read_buf(&mut self.buffer).await? {
                // The remote closed the connection. For this to be a clean
                // shutdown, there should be no data in the read buffer. If
                // there is data, this means the peer closed the socket while
                // sending a frame.
                if self.buffer.is_empty() {
                    return Ok(None);
                } else {
                    return Err("connection reset by peer".into());
                }
            }
        }
    }

    /// Write a single `Frame` value to the underlying stream.
    ///
    /// The `Frame` value is written to the socket using the various `write_*`
    /// functions provided by `AsyncWrite`. Calling these functions directly on
    /// a `TcpStream` is **not** advised s this will result in a large number of
    /// syscalls. However, it is fine to call these functions on a *buffered*
    /// write stream. The data will be written to the buffer. Once the buffer is
    /// full, it is flushed to the underlying socket.
    pub(crate) async fn write_frame(&mut self, frame: &Frame) -> io::Result<()> {
        // Arrays are encoded by encoding each entry. All other frame types are
        // considered literals. For now, mini-redis is not able to encoded
        // recursive frame structures. See below for more details.
        match frame {
            Frame::Array(val) => {
                // Encode the frame type prefix. For an array, it is `*`.
                self.stream.write_u8(b'*').await?;

                // Encode the length of the array.
                self.write_decimal(val.len() as u64).await?;

                // Iterate and encode each entry in the array.
                for entry in &**val {
                    self.write_value(entry).await?;
                }
            }
            // The frme type is a literal. Encode the value directly.
            _ => self.write_value(frame).await?,
        }

        // Ensure the encoded frame is written to the socket. The calls above
        // are to the buffered stream and writes. Calling `flush` writes the
        // remaining contents of the buffer to the socket.
        self.stream.flush().await
    }

    /// Write a frame literal to the stream
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
            // Encoding an `Array` from within a value cannot be done using a
            // recursive strategy. In general, async fns do not support
            // recursion. Mini-redis has not needed to encode nested arrays yet,
            // so for now it is skipped.
            Frame::Array(_val) => unreachable!(),
        }

        Ok(())
    }

    /// Write a decimal frame to the stream
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
