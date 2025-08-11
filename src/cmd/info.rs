use crate::{Connection, Frame, Parse};

use bytes::Bytes;
use tracing::{debug, instrument};

/// Get server information and statistics.
///
/// Returns various information about the server including metrics and statistics.
#[derive(Debug)]
pub struct Info {
    /// Optional section to return (not implemented yet)
    _section: Option<String>,
}

impl Info {
    /// Create a new `Info` command.
    pub fn new() -> Info {
        Info { _section: None }
    }

    /// Parse an `Info` instance from a received frame.
    ///
    /// The `Parse` argument provides a cursor-like API to read fields from the
    /// `Frame`. At this point, the entire frame has already been received from
    /// the socket.
    ///
    /// The `INFO` string has already been consumed.
    ///
    /// # Returns
    ///
    /// Returns the `Info` value on success. If the frame is malformed, `Err` is
    /// returned.
    ///
    /// # Format
    ///
    /// Expects an array frame containing one or two entries.
    ///
    /// ```text
    /// INFO [section]
    /// ```
    pub(crate) fn parse_frames(parse: &mut Parse) -> crate::Result<Info> {
        // The `INFO` string has already been consumed.
        // The next value is optional and represents a section name.
        let section = match parse.next_string() {
            Ok(s) => Some(s),
            Err(_) => None,
        };

        Ok(Info { _section: section })
    }

    /// Apply the `Info` command to the specified `Db` instance.
    ///
    /// The response is written to `dst`. This is called by the server in order
    /// to execute a received command.
    #[instrument(skip(self, db, dst))]
    pub(crate) async fn apply(self, db: &crate::Db, dst: &mut Connection) -> crate::Result<()> {
        // Get metrics from the database
        let metrics = db.get_metrics();
        let info_string = metrics.info_string();

        debug!(?info_string);

        // Write the response back to the client as a bulk string
        let response = Frame::Bulk(Bytes::from(info_string));
        dst.write_frame(&response).await?;

        Ok(())
    }

    /// Converts the command into an equivalent `Frame`.
    ///
    /// This is called by the client when encoding an `Info` command to send to
    /// the server.
    pub(crate) fn into_frame(self) -> Frame {
        let mut frame = Frame::array();
        frame.push_bulk(Bytes::from("info".as_bytes()));
        if let Some(section) = self._section {
            frame.push_bulk(Bytes::from(section.into_bytes()));
        }
        frame
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Frame;

    #[test]
    fn test_info_new() {
        let info = Info::new();
        assert!(info._section.is_none());
    }

    #[test]
    fn test_info_into_frame() {
        let info = Info::new();
        let frame = info.into_frame();
        
        if let Frame::Array(frames) = frame {
            assert_eq!(frames.len(), 1);
            if let Frame::Bulk(bytes) = &frames[0] {
                assert_eq!(bytes.as_ref(), b"info");
            } else {
                panic!("Expected bulk frame");
            }
        } else {
            panic!("Expected array frame");
        }
    }

    #[test]
    fn test_info_with_section_into_frame() {
        let mut info = Info::new();
        info._section = Some("server".to_string());
        let frame = info.into_frame();
        
        if let Frame::Array(frames) = frame {
            assert_eq!(frames.len(), 2);
            if let Frame::Bulk(bytes) = &frames[0] {
                assert_eq!(bytes.as_ref(), b"info");
            } else {
                panic!("Expected bulk frame");
            }
            if let Frame::Bulk(bytes) = &frames[1] {
                assert_eq!(bytes.as_ref(), b"server");
            } else {
                panic!("Expected bulk frame");
            }
        } else {
            panic!("Expected array frame");
        }
    }
} 