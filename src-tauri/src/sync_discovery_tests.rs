use super::*;
use std::net::{Ipv4Addr, UdpSocket};

fn interface(ip: Ipv4Addr, up: bool, p2p: bool) -> if_addrs::Interface {
    if_addrs::Interface {
        name: "fixture".into(),
        index: Some(2),
        is_p2p: p2p,
        oper_status: if up {
            if_addrs::IfOperStatus::Up
        } else {
            if_addrs::IfOperStatus::Down
        },
        addr: if_addrs::IfAddr::V4(if_addrs::Ifv4Addr {
            ip,
            netmask: Ipv4Addr::new(255, 255, 255, 0),
            prefixlen: 24,
            broadcast: Some(Ipv4Addr::new(
                ip.octets()[0],
                ip.octets()[1],
                ip.octets()[2],
                255,
            )),
        }),
        #[cfg(windows)]
        adapter_name: "fixture-adapter".into(),
    }
}

#[test]
fn interface_selection_refuses_public_down_point_to_point_and_excessive_sets() {
    let fixtures = vec![
        interface(Ipv4Addr::new(192, 168, 0, 10), true, false),
        interface(Ipv4Addr::new(8, 8, 8, 8), true, false),
        interface(Ipv4Addr::new(10, 0, 0, 1), false, false),
        interface(Ipv4Addr::new(10, 0, 0, 2), true, true),
        interface(Ipv4Addr::LOCALHOST, true, false),
    ];
    assert_eq!(select_networks(fixtures).unwrap(), vec![ethernet()]);
    assert_eq!(select_networks(vec![]), Err(Failure::NoPrivateNetwork));
    let many = (1..=17)
        .map(|i| interface(Ipv4Addr::new(10, 0, 0, i), true, false))
        .collect();
    assert_eq!(select_networks(many), Err(Failure::TooManyInterfaces));
}

fn ethernet() -> Network {
    Network {
        ip: "192.168.0.10".parse().unwrap(),
        mask: "255.255.255.0".parse().unwrap(),
        index: 2,
    }
}
fn resolved(addresses: &[IpAddr], port: u16) -> ResolvedService {
    ServiceInfo::new(
        SERVICE,
        "anonymous-peer",
        "anonymous-peer.local.",
        addresses,
        port,
        &[("v", "1")][..],
    )
    .unwrap()
    .as_resolved_service()
}

#[test]
fn ethernet_and_wifi_addresses_share_policy_without_using_adapter_names() {
    let pc = ethernet();
    assert!(pc.allows("192.168.0.2:49152".parse().unwrap()));
    for peer in [
        "192.168.1.2:49152",
        "8.8.8.8:49152",
        "127.0.0.1:49152",
        "0.0.0.0:49152",
        "224.0.0.251:5353",
        "192.168.0.0:49152",
        "192.168.0.255:49152",
        "192.168.0.10:49152",
        "[::1]:49152",
        "[::ffff:192.168.0.2]:49152",
    ] {
        assert!(!pc.allows(peer.parse().unwrap()), "{peer}");
    }
    assert_eq!(
        pc.bind_address(49152),
        "192.168.0.10:49152".parse().unwrap()
    );
    let global = Network {
        ip: "203.0.113.2".parse().unwrap(),
        mask: "255.255.255.0".parse().unwrap(),
        index: 2,
    };
    assert!(!global.allows("203.0.113.3:49152".parse().unwrap()));
}

#[test]
fn ipv6_private_networks_require_the_correct_prefix_and_link_local_interface_scope() {
    let local = Network {
        ip: "fe80::10".parse().unwrap(),
        mask: "ffff:ffff:ffff:ffff::".parse().unwrap(),
        index: 7,
    };
    assert_eq!(
        local.bind_address(49152),
        "[fe80::10%7]:49152".parse().unwrap()
    );
    assert!(local.allows("[fe80::20%7]:49152".parse().unwrap()));
    assert!(!local.allows("[fe80::20%8]:49152".parse().unwrap()));
    assert!(!local.allows("[2001:db8::20]:49152".parse().unwrap()));
    let private = Network {
        ip: "fd12:3456::10".parse().unwrap(),
        mask: "ffff:ffff:ffff:ffff::".parse().unwrap(),
        index: 7,
    };
    assert!(private.allows("[fd12:3456::20]:49152".parse().unwrap()));
    assert!(!private.allows("[fd12:3457::20]:49152".parse().unwrap()));
}

#[test]
fn announcements_are_anonymous_and_resolved_addresses_cannot_escape_the_local_network() {
    let net = ethernet();
    let first = advertisement(&[net.clone()], 49152).unwrap();
    let second = advertisement(&[net.clone()], 49152).unwrap();
    assert_ne!(first.get_fullname(), second.get_fullname());
    let name = first
        .get_fullname()
        .strip_suffix(&format!(".{SERVICE}"))
        .unwrap();
    assert!(name.starts_with("vto-"));
    assert_eq!(name.len(), 36);
    assert!(name[4..].bytes().all(|b| b.is_ascii_hexdigit()));
    assert_eq!(first.get_properties().len(), 1);
    assert_eq!(first.get_property_val_str("v"), Some("1"));
    assert_eq!(first.get_addresses().len(), 1);
    assert!(!first.is_addr_auto());
    // No daemon is needed to validate a candidate: policy is independent of discovery transport.
    let allowed = |info: &ResolvedService| candidate_addresses(std::slice::from_ref(&net), info);
    assert_eq!(
        allowed(&resolved(
            &["192.168.0.2".parse().unwrap(), "8.8.8.8".parse().unwrap()],
            49152
        )),
        vec!["192.168.0.2:49152".parse::<SocketAddr>().unwrap()]
    );
    assert!(allowed(&resolved(&["192.168.0.2".parse().unwrap()], 443)).is_empty());
    let mut many = Vec::new();
    for i in 1..=33 {
        many.push(IpAddr::V4(Ipv4Addr::new(192, 168, 0, i)));
    }
    assert!(allowed(&resolved(&many, 49152)).is_empty());
}

#[test]
fn an_owned_discovery_daemon_acknowledges_shutdown_and_does_not_keep_a_live_handle() {
    let socket = UdpSocket::bind("127.0.0.1:0").unwrap();
    let port = socket.local_addr().unwrap().port();
    drop(socket);
    let loopback = if_addrs::get_if_addrs()
        .unwrap()
        .into_iter()
        .find(|i| i.ip() == IpAddr::V4(Ipv4Addr::LOCALHOST))
        .unwrap();
    let net = Network {
        ip: loopback.ip(),
        mask: "255.0.0.0".parse().unwrap(),
        index: loopback.index.unwrap_or(0),
    };
    let daemon = ServiceDaemon::new_with_port(port).unwrap();
    let mut discovery = Discovery::from_daemon(daemon, vec![net]).unwrap();
    discovery.advertise(49152).unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        match discovery
            .monitor
            .as_ref()
            .unwrap()
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .unwrap()
        {
            DaemonEvent::Announce(_, _) => break,
            DaemonEvent::Error(_) => panic!("test-only loopback discovery could not announce"),
            _ => {}
        }
    }
    discovery.close().unwrap();
    assert_eq!(discovery.check(), Err(Failure::Stopped));
    assert_eq!(
        discovery
            .daemon
            .status()
            .unwrap()
            .recv_timeout(Duration::from_secs(1))
            .unwrap(),
        DaemonStatus::Shutdown
    );
    discovery.close().unwrap();
}

#[test]
fn an_owned_listener_releases_its_exact_bound_address_when_dropped() {
    let network = Network {
        ip: Ipv4Addr::LOCALHOST.into(),
        mask: "255.0.0.0".parse().unwrap(),
        index: 0,
    };
    let listeners = Listeners::bind(&[network]).unwrap();
    let address = listeners.sockets[0].1.local_addr().unwrap();
    assert!(address.ip().is_loopback());
    assert_eq!(listeners.port(), address.port());
    assert!(listeners.accept().unwrap().is_none());
    assert!(TcpListener::bind(address).is_err());
    drop(listeners);
    let rebound = TcpListener::bind(address).unwrap();
    assert_eq!(rebound.local_addr().unwrap(), address);
}
