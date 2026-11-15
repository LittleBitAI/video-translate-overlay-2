//! Anonymous DNS-SD on explicitly selected private interfaces. Discovery never authorizes a peer.
use mdns_sd::{
    DaemonEvent, DaemonStatus, IfKind, IfPredicate, Receiver, ResolvedService, ScopedIp,
    ServiceDaemon, ServiceEvent, ServiceInfo,
};
use std::{
    net::{IpAddr, SocketAddr, SocketAddrV6, TcpListener, TcpStream},
    time::{Duration, Instant},
};

pub(crate) const SERVICE: &str = "_vto-sync._tcp.local.";
const MAX_INTERFACES: usize = 16;
const MAX_ENDPOINTS: usize = 32;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Failure {
    NoPrivateNetwork,
    TooManyInterfaces,
    Unavailable,
    Stopped,
    Shutdown,
}
type Result<T> = std::result::Result<T, Failure>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Network {
    ip: IpAddr,
    mask: IpAddr,
    index: u32,
}
fn private(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v) => v.is_private() || v.is_link_local(),
        IpAddr::V6(v) => v.is_unique_local() || v.is_unicast_link_local(),
    }
}
impl Network {
    pub fn current() -> Result<Vec<Self>> {
        let interfaces = if_addrs::get_if_addrs().map_err(|_| Failure::Unavailable)?;
        select_networks(interfaces)
    }

    pub fn bind_address(&self, port: u16) -> SocketAddr {
        match self.ip {
            IpAddr::V4(v) => SocketAddr::new(v.into(), port),
            IpAddr::V6(v) => SocketAddrV6::new(
                v,
                port,
                0,
                if v.is_unicast_link_local() {
                    self.index
                } else {
                    0
                },
            )
            .into(),
        }
    }

    pub fn allows(&self, peer: SocketAddr) -> bool {
        if !private(self.ip) || !private(peer.ip()) || peer.ip() == self.ip {
            return false;
        }
        match (self.ip, self.mask, peer) {
            (IpAddr::V4(local), IpAddr::V4(mask), SocketAddr::V4(peer)) => {
                let bits = u32::from(*peer.ip());
                let mask = u32::from(mask);
                (bits & mask) == (u32::from(local) & mask)
                    && bits & !mask != 0
                    && bits | mask != u32::MAX
            }
            (IpAddr::V6(local), IpAddr::V6(mask), SocketAddr::V6(peer)) => {
                (!local.is_unicast_link_local()
                    || peer.scope_id() == 0
                    || peer.scope_id() == self.index)
                    && (u128::from(*peer.ip()) & u128::from(mask))
                        == (u128::from(local) & u128::from(mask))
            }
            _ => false,
        }
    }
}

fn select_networks(interfaces: Vec<if_addrs::Interface>) -> Result<Vec<Network>> {
    let mut nets = Vec::new();
    for interface in interfaces {
        if !interface.is_oper_up() || interface.is_p2p() || !private(interface.ip()) {
            continue;
        }
        let mask = match &interface.addr {
            if_addrs::IfAddr::V4(a) if (8..=32).contains(&a.prefixlen) => IpAddr::V4(a.netmask),
            if_addrs::IfAddr::V6(a) if (7..=128).contains(&a.prefixlen) => IpAddr::V6(a.netmask),
            _ => continue,
        };
        let net = Network {
            ip: interface.ip(),
            mask,
            index: interface.index.unwrap_or(0),
        };
        if matches!(net.ip,IpAddr::V6(v) if v.is_unicast_link_local()) && net.index == 0 {
            continue;
        }
        if !nets.contains(&net) {
            nets.push(net);
        }
    }
    nets.sort_by_key(|n| (n.ip, n.index));
    if nets.is_empty() {
        return Err(Failure::NoPrivateNetwork);
    }
    if nets.len() > MAX_INTERFACES {
        return Err(Failure::TooManyInterfaces);
    }
    Ok(nets)
}

/// One nonblocking listener per selected address, all sharing an ephemeral port.
/// The native session lease owns this set; dropping it closes every listening descriptor.
pub(crate) struct Listeners {
    sockets: Vec<(Network, TcpListener)>,
    port: u16,
}
impl Listeners {
    pub fn bind(networks: &[Network]) -> Result<Self> {
        if networks.is_empty() || networks.len() > MAX_INTERFACES {
            return Err(Failure::NoPrivateNetwork);
        }
        let mut owner = Self {
            sockets: Vec::new(),
            port: 0,
        };
        for network in networks {
            let listener = TcpListener::bind(network.bind_address(owner.port))
                .map_err(|_| Failure::Unavailable)?;
            listener
                .set_nonblocking(true)
                .map_err(|_| Failure::Unavailable)?;
            owner.port = listener
                .local_addr()
                .map_err(|_| Failure::Unavailable)?
                .port();
            owner.sockets.push((network.clone(), listener));
        }
        Ok(owner)
    }
    pub fn port(&self) -> u16 {
        self.port
    }
    pub fn accept(&self) -> Result<Option<TcpStream>> {
        for (network, listener) in &self.sockets {
            // A burst of refused sources must not keep the native owner from checking cancellation.
            for _ in 0..4 {
                match listener.accept() {
                    Ok((tcp, peer)) if network.allows(peer) => return Ok(Some(tcp)),
                    Ok(_) => continue,
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                    Err(_) => return Err(Failure::Unavailable),
                }
            }
        }
        Ok(None)
    }
}

/// The native session worker creates one daemon, shares it only inside its lease,
/// and explicitly closes it on account/network replacement or shutdown.
pub(crate) struct Discovery {
    daemon: ServiceDaemon,
    events: Option<Receiver<ServiceEvent>>,
    monitor: Option<Receiver<DaemonEvent>>,
    registration: Option<String>,
    networks: Vec<Network>,
    closed: bool,
}
impl Discovery {
    pub fn new(networks: Vec<Network>) -> Result<Self> {
        let daemon = ServiceDaemon::new().map_err(|_| Failure::Unavailable)?;
        Self::from_daemon(daemon, networks)
    }

    fn from_daemon(daemon: ServiceDaemon, networks: Vec<Network>) -> Result<Self> {
        // Establish the owner before any fallible configuration; a failure still shuts it down.
        let mut owner = Self {
            daemon,
            events: None,
            monitor: None,
            registration: None,
            networks,
            closed: false,
        };
        owner.monitor = Some(owner.daemon.monitor().map_err(|_| Failure::Unavailable)?);
        owner
            .daemon
            .disable_interface(IfKind::All)
            .map_err(|_| Failure::Unavailable)?;
        let allowed = owner.networks.clone();
        owner
            .daemon
            .enable_interface(IfKind::Predicate(IfPredicate::new(move |i| {
                i.is_oper_up()
                    && !i.is_p2p()
                    && allowed
                        .iter()
                        .any(|n| n.ip == i.ip() && (n.index == 0 || Some(n.index) == i.index))
            })))
            .map_err(|_| Failure::Unavailable)?;
        owner.events = Some(
            owner
                .daemon
                .browse(SERVICE)
                .map_err(|_| Failure::Unavailable)?,
        );
        Ok(owner)
    }

    pub fn advertise(&mut self, port: u16) -> Result<()> {
        if self.registration.is_some() || port < 1024 {
            return Err(Failure::Unavailable);
        }
        let info = advertisement(&self.networks, port)?;
        self.registration = Some(info.get_fullname().into());
        self.daemon.register(info).map_err(|_| Failure::Unavailable)
    }

    pub fn check(&self) -> Result<()> {
        if self.closed {
            return Err(Failure::Stopped);
        }
        for event in self
            .monitor
            .as_ref()
            .ok_or(Failure::Unavailable)?
            .try_iter()
        {
            if matches!(event, DaemonEvent::Error(_)) {
                return Err(Failure::Unavailable);
            }
        }
        if self
            .events
            .as_ref()
            .ok_or(Failure::Unavailable)?
            .is_disconnected()
        {
            return Err(Failure::Stopped);
        }
        Ok(())
    }

    pub fn next_event(&self) -> Result<Option<ServiceEvent>> {
        self.check()?;
        match self.events.as_ref().ok_or(Failure::Unavailable)?.try_recv() {
            Ok(event) => Ok(Some(event)),
            Err(mdns_sd::TryRecvError::Empty) => Ok(None),
            Err(_) => Err(Failure::Stopped),
        }
    }

    pub fn endpoints(&self, info: &ResolvedService) -> Vec<SocketAddr> {
        candidate_addresses(&self.networks, info)
    }

    pub fn close(&mut self) -> Result<()> {
        if self.closed {
            return Ok(());
        }
        if let Some(name) = self.registration.take() {
            let _ = self.daemon.unregister(&name);
        }
        let _ = self.daemon.stop_browse(SERVICE);
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            match self.daemon.shutdown() {
                Ok(receipt) => {
                    if receipt
                        .recv_timeout(deadline.saturating_duration_since(Instant::now()))
                        .ok()
                        == Some(DaemonStatus::Shutdown)
                    {
                        self.closed = true;
                        return Ok(());
                    }
                    return Err(Failure::Shutdown);
                }
                Err(mdns_sd::Error::DaemonShutdown) => {
                    self.closed = true;
                    return Ok(());
                }
                Err(mdns_sd::Error::Again) if Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_millis(10))
                }
                Err(_) => return Err(Failure::Shutdown),
            }
        }
    }
}
impl Drop for Discovery {
    fn drop(&mut self) {
        if self.close().is_err() {
            crate::log("LAN discovery shutdown was not acknowledged");
        }
    }
}

fn advertisement(networks: &[Network], port: u16) -> Result<ServiceInfo> {
    let mut random = [0u8; 16];
    getrandom::fill(&mut random).map_err(|_| Failure::Unavailable)?;
    let name = format!(
        "vto-{}",
        random
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    );
    let ips = networks.iter().map(|n| n.ip).collect::<Vec<_>>();
    ServiceInfo::new(
        SERVICE,
        &name,
        &format!("{name}.local."),
        &ips[..],
        port,
        &[("v", "1")][..],
    )
    .map_err(|_| Failure::Unavailable)
}

fn candidate_addresses(networks: &[Network], info: &ResolvedService) -> Vec<SocketAddr> {
    if info.ty_domain != SERVICE
        || info.get_port() < 1024
        || info.get_property_val_str("v") != Some("1")
        || info.get_addresses().len() > MAX_ENDPOINTS
    {
        return Vec::new();
    }
    let mut addresses = Vec::new();
    for ip in info.get_addresses() {
        let address = match ip {
            ScopedIp::V4(v) => SocketAddr::new((*v.addr()).into(), info.get_port()),
            ScopedIp::V6(v) => {
                if v.addr().is_unicast_link_local() && v.scope_id().index == 0 {
                    continue;
                }
                SocketAddrV6::new(
                    *v.addr(),
                    info.get_port(),
                    0,
                    if v.addr().is_unicast_link_local() {
                        v.scope_id().index
                    } else {
                        0
                    },
                )
                .into()
            }
            _ => continue,
        };
        if networks.iter().any(|n| n.allows(address)) && !addresses.contains(&address) {
            addresses.push(address);
        }
    }
    addresses.sort();
    addresses
}

#[cfg(test)]
#[path = "sync_discovery_tests.rs"]
mod tests;
