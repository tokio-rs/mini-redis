use mini_redis::frame::{Error, Frame};
use std::io::Cursor;

#[test]
fn check_rejects_invalid_negative_bulk_length() {
    // Valid null bulk string in RESP.
    let mut ok = Cursor::new(&b"$-1\r\n"[..]);
    assert!(Frame::check(&mut ok).is_ok());

    // Invalid negative bulk length.
    let mut bad = Cursor::new(&b"$-2\r\n"[..]);
    let err = Frame::check(&mut bad).unwrap_err();

    match err {
        Error::Other(e) => {
            assert_eq!(e.to_string(), "protocol error; invalid frame format");
        }
        Error::Incomplete => panic!("expected protocol error, got incomplete"),
    }
}
