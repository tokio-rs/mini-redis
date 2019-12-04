use crate::Frame;

use bytes::Bytes;
use std::{io, str, vec};

/// Utility for parsing a command
#[derive(Debug)]
pub(crate) struct Parse {
    parts: vec::IntoIter<Box<Frame>>,
}

#[derive(Debug)]
pub(crate) enum ParseError {
    EndOfStream,
    Invalid,
    UnknownCommand(String),
}

impl Parse {
    pub(crate) fn new(frame: Frame) -> Result<Parse, ParseError> {
        let array = match frame {
            Frame::Array(array) => array,
            _ => return Err(ParseError::Invalid),
        };

        Ok(Parse { parts: array.into_iter() })
    }

    fn next(&mut self) -> Result<Frame, ParseError> {
        self.parts.next()
            .map(|frame| *frame)
            .ok_or(ParseError::EndOfStream)
    }

    pub(crate) fn next_string(&mut self) -> Result<String, ParseError> {
        match self.next()? {
            Frame::Simple(s) => Ok(s),
            Frame::Bulk(data) => {
                str::from_utf8(&data[..])
                    .map(|s| s.to_string())
                    .map_err(|_| ParseError::Invalid)
            }
            _ => Err(ParseError::Invalid),
        }
    }

    pub(crate) fn next_bytes(&mut self) -> Result<Bytes, ParseError> {
        match self.next()? {
            Frame::Simple(s) => Ok(Bytes::from(s.into_bytes())),
            Frame::Bulk(data) => Ok(data),
            _ => Err(ParseError::Invalid),
        }
    }

    pub(crate) fn next_int(&mut self) -> Result<u64, ParseError> {
        match self.next()? {
            Frame::Integer(v) => Ok(v),
            _ => Err(ParseError::Invalid),
        }
    }

    /// Ensure there are no more entries in the array
    pub(crate) fn finish(&mut self) -> Result<(), ParseError> {
        if self.parts.next().is_none() {
            Ok(())
        } else {
            Err(ParseError::Invalid)
        }
    }
}

impl From<ParseError> for io::Error {
    fn from(src: ParseError) -> io::Error {
        use ParseError::*;

        io::Error::new(
            io::ErrorKind::Other,
            match src {
                EndOfStream => "end of stream".to_string(),
                Invalid => "invalid".to_string(),
                UnknownCommand(cmd) => format!("unknown command `{}`", cmd),
            })
    }
}
