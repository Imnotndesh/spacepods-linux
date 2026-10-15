//! Bounded newline-delimited frames for the IPC socket.
//!
//! `AsyncBufReadExt::read_line` grows its buffer without limit, so a client that never
//! sends a newline can exhaust daemon memory. This reader enforces [`MAX_FRAME_BYTES`]
//! and reports an oversized frame instead of buffering it.

use std::io;
use tokio::io::{AsyncBufRead, AsyncBufReadExt};

/// Largest accepted frame (one JSON message, without its trailing newline).
pub const MAX_FRAME_BYTES: usize = 64 * 1024;

/// Why a frame could not be read.
#[derive(Debug)]
pub enum FrameError {
    /// The frame exceeded [`MAX_FRAME_BYTES`]. The stream is no longer in a usable
    /// state and the connection should be closed.
    TooLarge,
    /// The peer closed the connection in the middle of a frame.
    UnexpectedEof,
    /// Underlying I/O error.
    Io(io::Error),
}

/// Read one newline-terminated frame into `buf` (without the newline).
///
/// Returns `Ok(false)` on a clean end of stream (no partial frame pending).
pub async fn read_frame<R>(reader: &mut R, buf: &mut Vec<u8>) -> Result<bool, FrameError>
where
    R: AsyncBufRead + Unpin,
{
    buf.clear();
    loop {
        let available = reader.fill_buf().await.map_err(FrameError::Io)?;
        if available.is_empty() {
            return if buf.is_empty() {
                Ok(false)
            } else {
                Err(FrameError::UnexpectedEof)
            };
        }

        match available.iter().position(|&b| b == b'\n') {
            Some(i) => {
                if buf.len() + i > MAX_FRAME_BYTES {
                    reader.consume(i + 1);
                    return Err(FrameError::TooLarge);
                }
                buf.extend_from_slice(&available[..i]);
                reader.consume(i + 1);
                return Ok(true);
            }
            None => {
                let n = available.len();
                if buf.len() + n > MAX_FRAME_BYTES {
                    reader.consume(n);
                    return Err(FrameError::TooLarge);
                }
                buf.extend_from_slice(available);
                reader.consume(n);
            }
        }
    }
}

/// Frames are UTF-8 JSON; convert a read frame to `&str`, rejecting invalid UTF-8.
pub fn frame_as_str(buf: &[u8]) -> Result<&str, io::Error> {
    std::str::from_utf8(buf).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncWriteExt, BufReader};

    #[tokio::test]
    async fn reads_lines_and_reports_clean_eof() {
        let (mut tx, rx) = tokio::io::duplex(1024);
        tokio::spawn(async move {
            tx.write_all(b"{\"a\":1}\n{\"b\":2}\n").await.unwrap();
        });
        let mut reader = BufReader::new(rx);
        let mut buf = Vec::new();
        assert!(read_frame(&mut reader, &mut buf).await.unwrap());
        assert_eq!(buf, b"{\"a\":1}");
        assert!(read_frame(&mut reader, &mut buf).await.unwrap());
        assert_eq!(buf, b"{\"b\":2}");
        assert!(!read_frame(&mut reader, &mut buf).await.unwrap());
    }

    #[tokio::test]
    async fn rejects_frame_over_limit_without_newline() {
        let (mut tx, rx) = tokio::io::duplex(MAX_FRAME_BYTES * 2);
        tokio::spawn(async move {
            let chunk = vec![b'x'; MAX_FRAME_BYTES + 1];
            let _ = tx.write_all(&chunk).await;
        });
        let mut reader = BufReader::new(rx);
        let mut buf = Vec::new();
        assert!(matches!(
            read_frame(&mut reader, &mut buf).await,
            Err(FrameError::TooLarge)
        ));
    }

    #[tokio::test]
    async fn rejects_frame_over_limit_with_newline() {
        let (mut tx, rx) = tokio::io::duplex(MAX_FRAME_BYTES * 2);
        tokio::spawn(async move {
            let mut data = vec![b'x'; MAX_FRAME_BYTES + 10];
            data.push(b'\n');
            let _ = tx.write_all(&data).await;
        });
        let mut reader = BufReader::new(rx);
        let mut buf = Vec::new();
        assert!(matches!(
            read_frame(&mut reader, &mut buf).await,
            Err(FrameError::TooLarge)
        ));
    }

    #[tokio::test]
    async fn eof_mid_frame_is_an_error() {
        let (mut tx, rx) = tokio::io::duplex(64);
        tokio::spawn(async move {
            tx.write_all(b"{\"partial\"").await.unwrap();
            drop(tx);
        });
        let mut reader = BufReader::new(rx);
        let mut buf = Vec::new();
        assert!(matches!(
            read_frame(&mut reader, &mut buf).await,
            Err(FrameError::UnexpectedEof)
        ));
    }
}
