use bytes::Bytes;
use std::time::Duration;

pub(crate) fn duration_from_ms_str(src: &str) -> Result<Duration, std::num::ParseIntError> {
    let millis = src.parse::<u64>()?;
    Ok(Duration::from_millis(millis))
}

pub(crate) fn bytes_from_str(src: &str) -> Bytes {
    Bytes::from(src.to_string())
}
