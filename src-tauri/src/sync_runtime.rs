//! The native app owns one controller. Its session lease owns discovery and listeners;
//! bounded connection workers own sockets and never own the controller or account manager.
use crate::{account::{AuthState, Session}, google_auth::AuthError, sync_channel::{Channel, Failure}, sync_discovery::{Discovery, Listeners, Network}, Core};
use mdns_sd::ServiceEvent;
use serde::Serialize;
use std::{
    collections::BTreeMap,
    net::{IpAddr, Shutdown, TcpStream},
    sync::{atomic::{AtomicBool, Ordering}, mpsc, Arc, Mutex, Weak},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

const MAX_CONNECTIONS: usize = 4;
const MAX_PEERS: usize = 16;
const POLL: Duration = Duration::from_millis(100);
const INTERVAL: Duration = Duration::from_secs(2);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ConnectionState {
    Disabled, SignedOut, AuthenticationRequired, NoPrivateNetwork, DiscoveryUnavailable,
    ListenerUnavailable, BackgroundPaused, Listening, Searching, Connected, Interrupted,
    AccountMismatch, Rejected, Stopped,
}
#[derive(Clone, Serialize)]
pub(crate) struct Snapshot {
    generation: u64,
    pub state: ConnectionState,
    last_success: Option<i64>,
}
impl Default for Snapshot {
    fn default() -> Self { Self { generation: 0, state: ConnectionState::Disabled, last_success: None } }
}
#[derive(Clone, Default)]
pub(crate) struct StatusStore(Arc<Mutex<Snapshot>>);
impl StatusStore {
    pub fn get(&self, generation: u64) -> Option<Snapshot> {
        self.0.lock().ok().filter(|s| s.generation == generation).map(|s| s.clone())
    }
    fn set(&self, generation: u64, state: ConnectionState, success: bool) {
        if let Ok(mut current) = self.0.lock() {
            if current.generation != generation { current.last_success = None; }
            current.generation = generation;
            current.state = state;
            if success { current.last_success = Some(crate::account::now()); }
        }
    }
}

#[derive(Default)]
struct Connections {
    stopped: AtomicBool,
    slots: Mutex<BTreeMap<u64, Option<TcpStream>>>,
}
impl Connections {
    fn reserve(self: &Arc<Self>, cancelled: Arc<AtomicBool>) -> Option<Slot> {
        let mut slots = self.slots.lock().ok()?;
        if self.stopped.load(Ordering::Acquire) || cancelled.load(Ordering::Acquire) || slots.len() >= MAX_CONNECTIONS { return None; }
        let id = (0..MAX_CONNECTIONS as u64).find(|id| !slots.contains_key(id))?;
        slots.insert(id, None);
        Some(Slot { owner: self.clone(), id, cancelled })
    }
    fn close(&self) {
        if let Ok(slots) = self.slots.lock() {
            for socket in slots.values().flatten() { let _ = socket.shutdown(Shutdown::Both); }
        }
    }
    fn stop(&self) { self.stopped.store(true, Ordering::Release); self.close(); }
}
struct Slot {
    owner: Arc<Connections>,
    id: u64,
    cancelled: Arc<AtomicBool>,
}
impl Slot {
    fn current(&self) -> bool { !self.owner.stopped.load(Ordering::Acquire) && !self.cancelled.load(Ordering::Acquire) }
    fn attach(&self, tcp: &TcpStream) -> Result<(), Failure> {
        let mut slots = self.owner.slots.lock().map_err(|_| Failure::Transport)?;
        if !self.current() { return Err(AuthError::Cancelled.into()); }
        *slots.get_mut(&self.id).ok_or(Failure::Transport)? = Some(tcp.try_clone().map_err(|_| Failure::Transport)?);
        Ok(())
    }
}
impl Drop for Slot {
    fn drop(&mut self) {
        if let Ok(mut slots) = self.owner.slots.lock() { slots.remove(&self.id); }
    }
}

struct Peer {
    addresses: Vec<std::net::SocketAddr>,
    next_address: usize,
    next: Instant,
    failures: u32,
    busy: bool,
}
impl Peer {
    fn finish(&mut self, success: bool) {
        self.busy = false;
        self.failures = if success { 0 } else { (self.failures + 1).min(5) };
        self.next = Instant::now() + Duration::from_secs(if success { 2 } else { 1u64 << self.failures });
    }
}
struct Lease {
    session: Arc<Session>,
    networks: Vec<Network>,
    discovery: Discovery,
    listeners: Option<Listeners>,
    cancelled: Arc<AtomicBool>,
    connections: Arc<Connections>,
    peers: BTreeMap<String, Peer>,
    incoming: BTreeMap<IpAddr, Instant>,
    last_success: Option<Instant>,
}
impl Lease {
    fn new(session: Arc<Session>, networks: Vec<Network>, server: bool, connections: Arc<Connections>) -> Result<Self, ConnectionState> {
        let listeners = if server { Some(Listeners::bind(&networks).map_err(|_| ConnectionState::ListenerUnavailable)?) } else { None };
        let mut discovery = Discovery::new(networks.clone()).map_err(|_| ConnectionState::DiscoveryUnavailable)?;
        if let Some(listeners) = &listeners { discovery.advertise(listeners.port()).map_err(|_| ConnectionState::DiscoveryUnavailable)?; }
        Ok(Self { session, networks, discovery, listeners, cancelled: Arc::new(AtomicBool::new(false)), connections,
            peers: BTreeMap::new(), incoming: BTreeMap::new(), last_success: None })
    }
    fn discoveries(&mut self) -> Result<(), ConnectionState> {
        for _ in 0..16 {
            match self.discovery.next_event().map_err(|_| ConnectionState::DiscoveryUnavailable)? {
                Some(ServiceEvent::ServiceResolved(info)) if self.listeners.is_none() => {
                    let addresses = self.discovery.endpoints(&info);
                    if addresses.is_empty() || info.fullname.len() > 253 { continue; }
                    if let Some(peer) = self.peers.get_mut(&info.fullname) { peer.addresses = addresses; }
                    else if self.peers.len() < MAX_PEERS {
                        self.peers.insert(info.fullname.clone(), Peer { addresses, next_address: 0, next: Instant::now(), failures:0, busy:false });
                    }
                },
                Some(ServiceEvent::ServiceRemoved(_, name)) => { self.peers.remove(&name); },
                Some(_) => {},
                None => break,
            }
        }
        Ok(())
    }
    fn incoming(&mut self) -> Result<Option<TcpStream>, ConnectionState> {
        let Some(listeners) = &self.listeners else { return Ok(None) };
        let Some(tcp) = listeners.accept().map_err(|_| ConnectionState::ListenerUnavailable)? else { return Ok(None) };
        let peer = tcp.peer_addr().map_err(|_| ConnectionState::ListenerUnavailable)?.ip();
        self.incoming.retain(|_, at| at.elapsed() < Duration::from_secs(30));
        if self.incoming.get(&peer).is_some_and(|at| at.elapsed() < INTERVAL)
            || (!self.incoming.contains_key(&peer) && self.incoming.len() >= 64) { return Ok(None); }
        self.incoming.insert(peer, Instant::now());
        Ok(Some(tcp))
    }
}
impl Drop for Lease {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Release);
        self.connections.close();
        if self.discovery.close().is_err() { crate::log("sync discovery shutdown was not acknowledged"); }
    }
}

struct Outcome {
    session: Arc<Session>,
    lease: Arc<AtomicBool>,
    name: Option<String>,
    result: Result<bool, Failure>,
}
fn connection(core: Arc<Core>, session: Arc<Session>, slot: Slot, server: bool, tcp: TcpStream) -> Result<bool, Failure> {
    slot.attach(&tcp)?;
    let mut channel = if server { Channel::server(tcp, session, &core.account)? } else { Channel::client(tcp, session, &core.account)? };
    if !slot.current() { return Err(AuthError::Cancelled.into()); }
    channel.exchange(&core.account)
}

pub(crate) struct Runtime {
    connections: Arc<Connections>,
    thread: Mutex<Option<JoinHandle<()>>>,
}
impl Runtime {
    pub fn start(core: &Arc<Core>, server: bool) -> Result<Self, String> {
        let mut bytes = [0u8; 16];
        getrandom::fill(&mut bytes).map_err(|e| e.to_string())?;
        let owner: String = bytes.iter().map(|byte| format!("{byte:02x}")).collect();
        core.svc.notifier.register_sync(&owner)?;
        let connections = Arc::new(Connections::default());
        let controls = connections.clone();
        let weak = Arc::downgrade(core);
        let status = core.sync.clone();
        let handle = thread::Builder::new().name("local-sync".into()).spawn(move || run(weak, controls, status, server, owner)).map_err(|e| e.to_string())?;
        Ok(Self { connections, thread: Mutex::new(Some(handle)) })
    }
    /// The caller may join on a background thread; never wait for DNS-SD shutdown on the UI thread.
    pub fn stop(&self) -> Option<JoinHandle<()>> {
        self.connections.stop();
        self.thread.lock().ok()?.take()
    }
}
impl Drop for Runtime {
    fn drop(&mut self) { self.connections.stop(); }
}

fn run(weak: Weak<Core>, connections: Arc<Connections>, status: StatusStore, server: bool, owner: String) {
    let (send, receive) = mpsc::sync_channel::<Outcome>(MAX_CONNECTIONS * 2);
    let mut lease: Option<Lease> = None;
    let mut network_check = Instant::now();
    let mut retry = Instant::now();
    let mut networks = Vec::new();
    let mut generation = 0;
    let mut background_generation = None;
    let mut background_ready = false;
    let mut background_check = Instant::now();
    let mut workers: Vec<JoinHandle<()>> = Vec::new();
    while !connections.stopped.load(Ordering::Acquire) {
        let mut running = Vec::new();
        for worker in workers.drain(..) {
            if worker.is_finished() {
                if worker.join().is_err() { crate::log("sync connection worker panicked"); }
            } else { running.push(worker); }
        }
        workers = running;
        let Some(core) = weak.upgrade() else { break };
        let Ok(account) = core.account.status() else {
            lease = None;
            if background_generation.take().is_some() { let _ = core.svc.notifier.sync_background(&owner, generation, false); }
            background_ready = false;
            thread::sleep(POLL); continue;
        };
        generation = account.generation;
        let session = core.account.session().ok().filter(|session| session.device().is_ok());
        if session.is_none() {
            lease = None;
            if background_generation.take().is_some() { let _ = core.svc.notifier.sync_background(&owner, generation, false); }
            background_ready = false;
            let state = if matches!(account.state, AuthState::SignedIn | AuthState::Expired) { ConnectionState::AuthenticationRequired } else { ConnectionState::SignedOut };
            status.set(generation, state, false);
            drop(core); thread::sleep(POLL); continue;
        }
        let session = session.unwrap();
        if background_generation != Some(generation) { background_check = Instant::now(); background_ready = false; }
        if Instant::now() >= background_check {
            background_generation = Some(generation);
            background_check = Instant::now() + INTERVAL;
            background_ready = core.svc.notifier.sync_background(&owner, generation, true).unwrap_or(false);
        }
        if !background_ready {
            lease = None;
            status.set(generation, ConnectionState::BackgroundPaused, false);
            drop(core); thread::sleep(POLL); continue;
        }
        if lease.as_ref().is_some_and(|live| !Arc::ptr_eq(&live.session, &session)) { lease = None; retry = Instant::now(); network_check = Instant::now(); }
        if Instant::now() >= network_check {
            network_check = Instant::now() + INTERVAL;
            match Network::current() {
                Ok(current) => {
                    if current != networks { lease = None; networks = current; retry = Instant::now(); }
                },
                Err(_) => {
                    lease = None; networks.clear();
                    status.set(generation, ConnectionState::NoPrivateNetwork, false);
                },
            }
        }
        if lease.is_none() && !networks.is_empty() && Instant::now() >= retry {
            retry = Instant::now() + Duration::from_secs(5);
            match Lease::new(session.clone(), networks.clone(), server, connections.clone()) {
                Ok(live) => {
                    lease = Some(live);
                    status.set(generation, if server { ConnectionState::Listening } else { ConnectionState::Searching }, false);
                    let _ = core.svc.browser.refresh_library();
                },
                Err(state) => status.set(generation, state, false),
            }
        }
        if let Some(live) = lease.as_mut() {
            if live.last_success.is_some_and(|at| at.elapsed() >= Duration::from_secs(30))
                && status.get(generation).is_some_and(|snapshot| snapshot.state == ConnectionState::Connected) {
                status.set(generation, if server { ConnectionState::Listening } else { ConnectionState::Searching }, false);
            }
            for outcome in receive.try_iter().take(MAX_CONNECTIONS * 2) {
                if !Arc::ptr_eq(&outcome.session, &live.session) || !Arc::ptr_eq(&outcome.lease, &live.cancelled) { continue; }
                let success = outcome.result.is_ok();
                if let Some(name) = outcome.name { if let Some(peer) = live.peers.get_mut(&name) { peer.finish(success); } }
                if success {
                    live.last_success = Some(Instant::now());
                    status.set(generation, ConnectionState::Connected, true);
                    if outcome.result == Ok(true) { let _ = core.svc.browser.refresh_library(); }
                } else if live.last_success.is_none_or(|at| at.elapsed() >= Duration::from_secs(30)) {
                    let state = match outcome.result {
                        Err(Failure::Auth(AuthError::Expired)) => ConnectionState::AuthenticationRequired,
                        Err(Failure::Auth(AuthError::AccountMismatch)) => ConnectionState::AccountMismatch,
                        Err(Failure::Auth(AuthError::Invalid)) | Err(Failure::Protocol) => ConnectionState::Rejected,
                        _ if !server => ConnectionState::Interrupted,
                        _ => ConnectionState::Listening,
                    };
                    status.set(generation, state, false);
                }
            }
            if let Err(state) = live.discoveries() { lease = None; retry = Instant::now() + Duration::from_secs(5); status.set(generation, state, false); }
        }
        let incoming = lease.as_mut().map(|live| live.incoming()).unwrap_or(Ok(None));
        if let Err(state) = incoming { lease = None; retry = Instant::now() + Duration::from_secs(5); status.set(generation, state, false); }
        else if let Some(live) = lease.as_mut() {
            if let Some(tcp) = incoming.unwrap() {
                if let Some(slot) = connections.reserve(live.cancelled.clone()) {
                    let owner = core.clone(); let session = live.session.clone(); let complete = send.clone();
                    let lease = live.cancelled.clone();
                    if let Ok(worker) = thread::Builder::new().name("sync-peer".into()).spawn(move || {
                        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| connection(owner, session.clone(), slot, true, tcp)))
                            .unwrap_or(Err(Failure::Transport));
                        let _ = complete.try_send(Outcome {session, lease, name:None, result});
                    }) { workers.push(worker); }
                }
            }
            for (name, peer) in live.peers.iter_mut().filter(|(_, peer)| !peer.busy && Instant::now() >= peer.next).take(MAX_CONNECTIONS) {
                let Some(slot) = connections.reserve(live.cancelled.clone()) else { break };
                let address = peer.addresses[peer.next_address % peer.addresses.len()];
                peer.next_address = peer.next_address.wrapping_add(1);
                let Some(network) = live.networks.iter().find(|network| network.allows(address)).cloned() else { peer.finish(false); continue };
                peer.busy = true;
                let name = name.clone(); let session = live.session.clone(); let owner = core.clone(); let complete = send.clone();
                let lease = live.cancelled.clone();
                let spawn = thread::Builder::new().name("sync-connect".into()).spawn(move || {
                    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        if !slot.current() { return Err(AuthError::Cancelled.into()); }
                        network.connect(address).map_err(|_| Failure::Transport)
                            .and_then(|tcp| connection(owner, session.clone(), slot, false, tcp))
                    })).unwrap_or(Err(Failure::Transport));
                    let _ = complete.try_send(Outcome {session, lease, name:Some(name), result});
                });
                match spawn { Ok(worker) => workers.push(worker), Err(_) => peer.finish(false) }
            }
        }
        drop(core); thread::sleep(POLL);
    }
    connections.stop();
    drop(lease);
    for worker in workers { if worker.join().is_err() { crate::log("sync connection worker panicked during shutdown"); } }
    if let Some(core) = weak.upgrade() { let _ = core.svc.notifier.sync_background(&owner, generation, false); }
    status.set(generation, ConnectionState::Stopped, false);
}

#[cfg(test)]
#[path = "sync_runtime_tests.rs"]
mod tests;
