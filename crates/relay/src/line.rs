//! One connection's incoming side, the same for a PC and a phone: text messages, with the idle
//! timeout, the pings, the budget of messages and bytes per minute, and the kill switch the hub
//! pulls on a peer that does not read.

use std::time::Duration;

use futures_util::stream::SplitStream;
use futures_util::StreamExt as _;
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::WebSocketStream;

use crate::hub::Link;
use crate::limits::{Limits, Window};

/// What came next on a connection.
pub(crate) enum Next {
    /// One text message.
    Text(String),
    /// The connection is over: closed by the peer, broken, idle too long, or killed.
    Gone,
    /// The peer sent more than its budget allows this minute.
    TooFast,
}

pub(crate) struct Line {
    stream: SplitStream<WebSocketStream<TcpStream>>,
    pub link: Link,
    idle: Duration,
    ping_every: Duration,
    messages_per_minute: u64,
    bytes_per_minute: u64,
    messages: Window,
    bytes: Window,
    clock: crate::Clock,
}

impl Line {
    pub fn new(
        stream: SplitStream<WebSocketStream<TcpStream>>,
        link: Link,
        limits: &Limits,
        clock: crate::Clock,
    ) -> Self {
        Self {
            stream,
            link,
            idle: limits.idle,
            ping_every: limits.ping_every,
            messages_per_minute: u64::from(limits.messages_per_minute),
            bytes_per_minute: limits.bytes_per_minute as u64,
            messages: Window::default(),
            bytes: Window::default(),
            clock,
        }
    }

    /// The connection's first text message, within `wait` of now, or it is gone: a peer that
    /// only answers pings and never says hello (or shows a pass) does not get to stay.
    pub async fn first(&mut self, wait: Duration) -> Next {
        self.read(wait, false).await
    }

    /// The next text message. A connection quiet for the idle time (nothing at all, not even a
    /// pong) is gone; anything it sends starts the idle time again.
    pub async fn next(&mut self) -> Next {
        let idle = self.idle;
        self.read(idle, true).await
    }

    /// Pings go out meanwhile; anything but text is passed over (and, when `restart`, keeps the
    /// connection alive).
    async fn read(&mut self, wait: Duration, restart: bool) -> Next {
        let deadline = tokio::time::sleep(wait);
        tokio::pin!(deadline);
        let mut ping = tokio::time::interval(self.ping_every);
        ping.reset();
        loop {
            tokio::select! {
                message = self.stream.next() => match message {
                    Some(Ok(Message::Text(text))) => {
                        let now = (self.clock)();
                        if self.messages.bump(now, 1) > self.messages_per_minute
                            || self.bytes.bump(now, text.len() as u64) > self.bytes_per_minute
                        {
                            return Next::TooFast;
                        }
                        return Next::Text(text.as_str().to_owned());
                    }
                    Some(Ok(Message::Close(_))) | None => {
                        log::debug!("line closed by the peer");
                        return Next::Gone;
                    }
                    // A frame too big, or not WebSocket: over.
                    Some(Err(e)) => {
                        log::debug!("line broke: {e}");
                        return Next::Gone;
                    }
                    // Binary frames are not in the contract; pings and pongs are answered or
                    // ignored by the library. Each counts as one message, and shows the peer is
                    // still there.
                    Some(Ok(other)) => {
                        let now = (self.clock)();
                        if self.messages.bump(now, 1) > self.messages_per_minute
                            || self.bytes.bump(now, other.len() as u64) > self.bytes_per_minute
                        {
                            return Next::TooFast;
                        }
                        if restart {
                            deadline.as_mut().reset(tokio::time::Instant::now() + wait);
                        }
                    }
                },
                _ = ping.tick() => self.link.send_message(Message::Ping(Vec::new().into())),
                _ = self.link.kill.notified() => {
                    log::debug!("line killed: the peer did not read, or its writer ended");
                    return Next::Gone;
                }
                _ = &mut deadline => {
                    log::debug!("line quiet for {wait:?}: closed");
                    return Next::Gone;
                }
            }
        }
    }
}
