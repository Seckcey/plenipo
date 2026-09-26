//! Bounded output capture: line splitting with a per-line cap and a ring buffer.

use std::collections::VecDeque;

use tokio::io::{AsyncBufReadExt, AsyncRead, BufReader};
use tokio::sync::mpsc;

use crate::dto::{OutputLine, OutputStream};

/// A line read from a child stream before it is sequenced.
#[derive(Debug)]
pub(crate) struct RawLine {
    pub stream: OutputStream,
    pub text: String,
    pub truncated: bool,
}

/// Read `reader` line by line and forward each line. Lines longer than `max_len` bytes are
/// cut and flagged; memory use per stream stays bounded even without newlines.
pub(crate) async fn read_lines<R: AsyncRead + Unpin>(
    reader: R,
    stream: OutputStream,
    max_len: usize,
    tx: mpsc::Sender<RawLine>,
) {
    let mut reader = BufReader::new(reader);
    let mut buf: Vec<u8> = Vec::new();
    let mut truncated = false;

    let append = |buf: &mut Vec<u8>, truncated: &mut bool, bytes: &[u8]| {
        let room = max_len.saturating_sub(buf.len());
        buf.extend_from_slice(&bytes[..bytes.len().min(room)]);
        if bytes.len() > room {
            *truncated = true;
        }
    };

    while let Ok(available) = reader.fill_buf().await {
        if available.is_empty() {
            break;
        }
        let (chunk_len, consumed, newline) = match available.iter().position(|&b| b == b'\n') {
            Some(pos) => (pos, pos + 1, true),
            None => (available.len(), available.len(), false),
        };
        append(&mut buf, &mut truncated, &available[..chunk_len]);
        reader.consume(consumed);
        if newline
            && tx
                .send(finish(stream, &mut buf, &mut truncated))
                .await
                .is_err()
        {
            return;
        }
    }
    if !buf.is_empty() || truncated {
        let _ = tx.send(finish(stream, &mut buf, &mut truncated)).await;
    }
}

fn finish(stream: OutputStream, buf: &mut Vec<u8>, truncated: &mut bool) -> RawLine {
    if buf.last() == Some(&b'\r') {
        buf.pop();
    }
    let line = RawLine {
        stream,
        text: String::from_utf8_lossy(buf).into_owned(),
        truncated: *truncated,
    };
    buf.clear();
    *truncated = false;
    line
}

/// Most recent output lines of one execution.
#[derive(Debug)]
pub(crate) struct OutputBuffer {
    lines: VecDeque<OutputLine>,
    capacity: usize,
    dropped: u64,
    next_seq: u64,
}

impl OutputBuffer {
    pub fn new(capacity: usize) -> Self {
        Self {
            lines: VecDeque::with_capacity(capacity.min(1024)),
            capacity: capacity.max(1),
            dropped: 0,
            next_seq: 1,
        }
    }

    pub fn push(&mut self, raw: RawLine, ts: u64) -> OutputLine {
        let line = OutputLine {
            seq: self.next_seq,
            stream: raw.stream,
            text: raw.text,
            truncated: raw.truncated,
            ts,
        };
        self.next_seq += 1;
        if self.lines.len() == self.capacity {
            self.lines.pop_front();
            self.dropped += 1;
        }
        self.lines.push_back(line.clone());
        line
    }

    pub fn snapshot(&self) -> (Vec<OutputLine>, u64) {
        (self.lines.iter().cloned().collect(), self.dropped)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn collect(input: &[u8], max_len: usize) -> Vec<RawLine> {
        let (tx, mut rx) = mpsc::channel(64);
        read_lines(input, OutputStream::Stdout, max_len, tx).await;
        let mut out = vec![];
        while let Some(l) = rx.recv().await {
            out.push(l);
        }
        out
    }

    #[tokio::test]
    async fn splits_lines_and_strips_crlf() {
        let lines = collect(b"one\r\ntwo\nthree", 100).await;
        let texts: Vec<_> = lines.iter().map(|l| l.text.as_str()).collect();
        assert_eq!(texts, ["one", "two", "three"]);
        assert!(lines.iter().all(|l| !l.truncated));
    }

    #[tokio::test]
    async fn caps_long_lines() {
        let mut input = vec![b'a'; 50];
        input.extend_from_slice(b"\nok\n");
        let lines = collect(&input, 10).await;
        assert_eq!(lines[0].text, "a".repeat(10));
        assert!(lines[0].truncated);
        assert_eq!(lines[1].text, "ok");
        assert!(!lines[1].truncated);
    }

    #[tokio::test]
    async fn invalid_utf8_is_replaced_not_fatal() {
        let lines = collect(b"bad \xff byte\n", 100).await;
        assert!(lines[0].text.starts_with("bad "));
        assert!(lines[0].text.contains('\u{fffd}'));
    }

    #[test]
    fn ring_buffer_is_bounded_and_sequenced() {
        let mut buf = OutputBuffer::new(3);
        for i in 0..5 {
            buf.push(
                RawLine {
                    stream: OutputStream::Stdout,
                    text: i.to_string(),
                    truncated: false,
                },
                0,
            );
        }
        let (lines, dropped) = buf.snapshot();
        assert_eq!(dropped, 2);
        assert_eq!(lines.iter().map(|l| l.seq).collect::<Vec<_>>(), [3, 4, 5]);
    }
}
