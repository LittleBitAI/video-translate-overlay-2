//! A fixed, bounded protocol over an owned TCP connection; no HTTP or Tauri dispatch.
use crate::{
    account::{Manager, Session},
    google_auth::{AuthError, Platform},
    sync::{Page, MAX_PAGE_BYTES},
    sync_tls::{self, PeerCertificate},
};
use rustls::{ClientConnection, ServerConnection, StreamOwned};
use serde::{Deserialize, Serialize};
use std::{
    io::{self, Read, Write},
    net::TcpStream,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

const CONTROL_LIMIT: usize = 16 * 1024;
const PAGE_LIMIT: usize = MAX_PAGE_BYTES + 1024;
const HANDSHAKE_TIME: Duration = Duration::from_secs(5);
const FRAME_TIME: Duration = Duration::from_secs(30);

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Failure {
    Auth(AuthError),
    Transport,
    Protocol,
}
impl From<AuthError> for Failure {
    fn from(e: AuthError) -> Self {
        Self::Auth(e)
    }
}
type Result<T> = std::result::Result<T, Failure>;

#[derive(Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
enum Frame {
    FreshGoogleRequired,
    NeedProof {
        fresh: bool,
    },
    Proof {
        token: Option<String>,
        platform: Platform,
        replica: String,
    },
    Cursor {
        through: i64,
    },
    Page {
        page: Page,
    },
    Committed {
        server: i64,
        client: i64,
    },
}

/// The connection worker owns the socket. Dropping it closes the descriptor;
/// cancellation is checked at most 250 ms apart while native I/O is blocked.
struct DeadlineIo {
    tcp: TcpStream,
    cancelled: Arc<AtomicBool>,
    deadline: Instant,
}
impl DeadlineIo {
    fn new(tcp: TcpStream, session: &Session) -> io::Result<Self> {
        tcp.set_read_timeout(Some(Duration::from_millis(250)))?;
        tcp.set_write_timeout(Some(Duration::from_millis(250)))?;
        Ok(Self {
            tcp,
            cancelled: session.cancellation(),
            deadline: Instant::now() + HANDSHAKE_TIME,
        })
    }
    fn require(&self) -> io::Result<()> {
        if self.cancelled.load(Ordering::Acquire) {
            return Err(io::ErrorKind::ConnectionAborted.into());
        }
        if Instant::now() >= self.deadline {
            return Err(io::ErrorKind::TimedOut.into());
        }
        Ok(())
    }
}
impl Read for DeadlineIo {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        loop {
            self.require()?;
            match self.tcp.read(buf) {
                Err(e)
                    if matches!(
                        e.kind(),
                        io::ErrorKind::TimedOut
                            | io::ErrorKind::WouldBlock
                            | io::ErrorKind::Interrupted
                    ) => {}
                result => return result,
            }
        }
    }
}
impl Write for DeadlineIo {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        loop {
            self.require()?;
            match self.tcp.write(buf) {
                Err(e)
                    if matches!(
                        e.kind(),
                        io::ErrorKind::TimedOut
                            | io::ErrorKind::WouldBlock
                            | io::ErrorKind::Interrupted
                    ) => {}
                result => return result,
            }
        }
    }
    fn flush(&mut self) -> io::Result<()> {
        self.require()?;
        self.tcp.flush()
    }
}

enum Tls {
    Client(StreamOwned<ClientConnection, DeadlineIo>),
    Server(StreamOwned<ServerConnection, DeadlineIo>),
}
impl Tls {
    fn rearm(&mut self, timeout: Duration) {
        let io = match self {
            Self::Client(s) => &mut s.sock,
            Self::Server(s) => &mut s.sock,
        };
        io.deadline = Instant::now() + timeout;
    }
    fn peer(&self) -> Result<PeerCertificate> {
        match self {
            Self::Client(s) => PeerCertificate::from_tls(&s.conn),
            Self::Server(s) => PeerCertificate::from_tls(&s.conn),
        }
        .map_err(Into::into)
    }
}
impl Read for Tls {
    fn read(&mut self, b: &mut [u8]) -> io::Result<usize> {
        match self {
            Self::Client(s) => s.read(b),
            Self::Server(s) => s.read(b),
        }
    }
}
impl Write for Tls {
    fn write(&mut self, b: &[u8]) -> io::Result<usize> {
        match self {
            Self::Client(s) => s.write(b),
            Self::Server(s) => s.write(b),
        }
    }
    fn flush(&mut self) -> io::Result<()> {
        match self {
            Self::Client(s) => s.flush(),
            Self::Server(s) => s.flush(),
        }
    }
}

fn read(stream: &mut impl Read, limit: usize) -> Result<Frame> {
    let mut size = [0u8; 4];
    stream
        .read_exact(&mut size)
        .map_err(|_| Failure::Transport)?;
    let size = u32::from_be_bytes(size) as usize;
    if size == 0 || size > limit {
        return Err(Failure::Protocol);
    }
    let mut body = vec![0u8; size];
    stream
        .read_exact(&mut body)
        .map_err(|_| Failure::Transport)?;
    serde_json::from_slice(&body).map_err(|_| Failure::Protocol)
}
fn write(stream: &mut impl Write, frame: &Frame) -> Result<()> {
    // Only typed proof/control/library frames cross this exit; never reusable secrets or commands.
    let limit = match frame {
        Frame::Proof { token, replica, .. } => {
            if token
                .as_ref()
                .is_some_and(|t| t.is_empty() || t.len() > 8192)
                || !replica_valid(replica)
            {
                return Err(Failure::Protocol);
            }
            CONTROL_LIMIT
        }
        Frame::Page { page } => {
            page.encode().map_err(|_| Failure::Protocol)?;
            PAGE_LIMIT
        }
        _ => CONTROL_LIMIT,
    };
    let body = serde_json::to_vec(frame).map_err(|_| Failure::Protocol)?;
    if body.len() > limit {
        return Err(Failure::Protocol);
    }
    stream
        .write_all(&(body.len() as u32).to_be_bytes())
        .and_then(|_| stream.write_all(&body))
        .and_then(|_| stream.flush())
        .map_err(|_| Failure::Transport)
}
fn replica_valid(replica: &str) -> bool {
    replica.len() == 32
        && replica
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// Construction owns one TLS socket and authenticates before returning access
/// to transfer operations. The worker holds the session; no manager/socket cycle.
pub(crate) struct Channel {
    stream: Tls,
    session: Arc<Session>,
    peer_replica: String,
    server: bool,
    authorized_until: Option<i64>,
}
impl Channel {
    pub fn client(tcp: TcpStream, session: Arc<Session>, manager: &Manager) -> Result<Self> {
        manager.with_session(&session, |_| Ok(()))?;
        let config = session.device()?.client_config()?;
        let io = DeadlineIo::new(tcp, &session).map_err(|_| Failure::Transport)?;
        let mut tls = StreamOwned::new(
            ClientConnection::new(config, sync_tls::server_name())
                .map_err(|_| Failure::Protocol)?,
            io,
        );
        while tls.conn.is_handshaking() {
            tls.conn
                .complete_io(&mut tls.sock)
                .map_err(|_| Failure::Transport)?;
        }
        let mut channel = Self {
            stream: Tls::Client(tls),
            session,
            peer_replica: String::new(),
            server: false,
            authorized_until: None,
        };
        channel.authenticate(manager)?;
        Ok(channel)
    }
    pub fn server(tcp: TcpStream, session: Arc<Session>, manager: &Manager) -> Result<Self> {
        manager.with_session(&session, |_| Ok(()))?;
        let config = session.device()?.server_config()?;
        let io = DeadlineIo::new(tcp, &session).map_err(|_| Failure::Transport)?;
        let mut tls = StreamOwned::new(
            ServerConnection::new(config).map_err(|_| Failure::Protocol)?,
            io,
        );
        while tls.conn.is_handshaking() {
            tls.conn
                .complete_io(&mut tls.sock)
                .map_err(|_| Failure::Transport)?;
        }
        let mut channel = Self {
            stream: Tls::Server(tls),
            session,
            peer_replica: String::new(),
            server: true,
            authorized_until: None,
        };
        channel.authenticate(manager)?;
        Ok(channel)
    }
    fn read(&mut self, limit: usize) -> Result<Frame> {
        self.require_current()?;
        self.stream.rearm(self.remaining());
        let result = read(&mut self.stream, limit);
        self.require_current()?;
        result
    }
    fn write(&mut self, frame: Frame) -> Result<()> {
        self.require_current()?;
        self.stream.rearm(self.remaining());
        let result = write(&mut self.stream, &frame);
        self.require_current()?;
        result
    }
    fn require_current(&self) -> Result<()> {
        self.session.require_current()?;
        if self
            .authorized_until
            .is_some_and(|until| until <= crate::account::now())
        {
            return Err(AuthError::Expired.into());
        }
        Ok(())
    }
    fn remaining(&self) -> Duration {
        let until = self
            .authorized_until
            .unwrap_or(self.session.identity.expires_at())
            .min(self.session.identity.expires_at());
        FRAME_TIME.min(Duration::from_secs(
            until.saturating_sub(crate::account::now()).max(0) as u64,
        ))
    }
    fn send_proof(&mut self, fresh: bool) -> Result<()> {
        let token = if fresh {
            match self.session.peer_proof() {
                Ok((proof, _)) => Some(proof.to_string()),
                Err(AuthError::Expired) => {
                    self.write(Frame::FreshGoogleRequired)?;
                    return Err(AuthError::Expired.into());
                }
                Err(e) => return Err(e.into()),
            }
        } else {
            None
        };
        self.write(Frame::Proof {
            token,
            platform: self.session.platform(),
            replica: self.session.journal.replica().into(),
        })
    }
    fn accept_proof(&mut self, manager: &Manager) -> Result<()> {
        let frame = self.read(CONTROL_LIMIT)?;
        if matches!(frame, Frame::FreshGoogleRequired) {
            return Err(AuthError::Expired.into());
        }
        let Frame::Proof {
            token,
            platform,
            replica,
        } = frame
        else {
            return Err(Failure::Protocol);
        };
        if !replica_valid(&replica) || replica == self.session.journal.replica() {
            return Err(Failure::Protocol);
        }
        let certificate = self.stream.peer()?;
        let until = match token {
            Some(token) => manager.enroll_peer(
                &self.session,
                &certificate.verify_google(manager.verifier(), &token, platform)?,
            )?,
            None => manager
                .peer_trust(&self.session, &certificate)?
                .ok_or(AuthError::Expired)?,
        };
        manager.with_session(&self.session, |_| Ok(()))?;
        self.peer_replica = replica;
        self.authorized_until = Some(until);
        Ok(())
    }
    fn authenticate(&mut self, manager: &Manager) -> Result<()> {
        if self.server {
            let fresh = self.requested_proof()?;
            self.send_proof(fresh)?;
            self.write(Frame::NeedProof {
                fresh: manager
                    .peer_trust(&self.session, &self.stream.peer()?)?
                    .is_none(),
            })?;
            self.accept_proof(manager)?;
        } else {
            self.write(Frame::NeedProof {
                fresh: manager
                    .peer_trust(&self.session, &self.stream.peer()?)?
                    .is_none(),
            })?;
            self.accept_proof(manager)?;
            let fresh = self.requested_proof()?;
            self.send_proof(fresh)?;
        }
        Ok(())
    }
    fn requested_proof(&mut self) -> Result<bool> {
        match self.read(CONTROL_LIMIT)? {
            Frame::NeedProof { fresh } => Ok(fresh),
            _ => Err(Failure::Protocol),
        }
    }
    fn cursor(&mut self) -> Result<i64> {
        match self.read(CONTROL_LIMIT)? {
            Frame::Cursor { through } if through >= 0 => Ok(through),
            _ => Err(Failure::Protocol),
        }
    }
    fn page(&mut self) -> Result<Page> {
        match self.read(PAGE_LIMIT)? {
            Frame::Page { page } => Ok(page),
            _ => Err(Failure::Protocol),
        }
    }
    fn outgoing(&self, manager: &Manager, after: i64) -> Result<Page> {
        self.require_current()?;
        manager
            .with_session(&self.session, |journal| journal.page(after))
            .map_err(Into::into)
    }
    fn receive(&self, manager: &Manager, page: &Page) -> Result<bool> {
        self.require_current()?;
        manager
            .with_session(&self.session, |journal| {
                journal.receive(&self.peer_replica, page)
            })
            .map_err(Into::into)
    }
    fn saved_cursor(&self, manager: &Manager) -> Result<i64> {
        self.require_current()?;
        manager
            .with_session(&self.session, |journal| journal.cursor(&self.peer_replica))
            .map_err(Into::into)
    }

    pub fn exchange(&mut self, manager: &Manager) -> Result<bool> {
        if self.server {
            let after = self.cursor()?;
            let outgoing = self.outgoing(manager, after)?;
            let sent = outgoing.through;
            self.write(Frame::Page { page: outgoing })?;
            self.write(Frame::Cursor {
                through: self.saved_cursor(manager)?,
            })?;
            let incoming = self.page()?;
            let changed = self.receive(manager, &incoming)?;
            self.write(Frame::Committed {
                server: sent,
                client: incoming.through,
            })?;
            Ok(changed)
        } else {
            self.write(Frame::Cursor {
                through: self.saved_cursor(manager)?,
            })?;
            let incoming = self.page()?;
            let changed = self.receive(manager, &incoming)?;
            let after = self.cursor()?;
            let outgoing = self.outgoing(manager, after)?;
            let sent = outgoing.through;
            self.write(Frame::Page { page: outgoing })?;
            match self.read(CONTROL_LIMIT)? {
                Frame::Committed { server, client }
                    if server == incoming.through && client == sent =>
                {
                    Ok(changed)
                }
                _ => Err(Failure::Protocol),
            }
        }
    }
}

#[cfg(test)]
#[path = "sync_channel_tests.rs"]
mod tests;
