use super::*;
use std::{io::Read, net::TcpListener};

#[test]
fn connection_ownership_caps_pending_workers_and_shutdown_closes_the_actual_socket() {
    let connections = Arc::new(Connections::default());
    let cancellation = Arc::new(AtomicBool::new(false));
    let first = connections.reserve(cancellation.clone()).unwrap();
    let mut slots = vec![first];
    for _ in 1..MAX_CONNECTIONS { slots.push(connections.reserve(cancellation.clone()).unwrap()); }
    assert!(connections.reserve(cancellation.clone()).is_none());
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let tcp = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
    let mut peer = listener.accept().unwrap().0;
    peer.set_read_timeout(Some(Duration::from_secs(1))).unwrap();
    slots[0].attach(&tcp).unwrap();
    connections.stop();
    assert_eq!(peer.read(&mut [0]).unwrap(), 0);
    assert!(connections.reserve(cancellation).is_none());
    assert!(matches!(slots[1].attach(&tcp), Err(Failure::Auth(AuthError::Cancelled))));
    drop(slots);
    assert!(connections.slots.lock().unwrap().is_empty());
}

#[test]
fn retiring_a_network_lease_rejects_pending_attachment_without_reassigning_its_slot() {
    let connections = Arc::new(Connections::default());
    let old = Arc::new(AtomicBool::new(false));
    let slot = connections.reserve(old.clone()).unwrap();
    old.store(true, Ordering::Release);
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let tcp = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
    let _peer = listener.accept().unwrap();
    assert!(matches!(slot.attach(&tcp), Err(Failure::Auth(AuthError::Cancelled))));
    assert!(!slot.current());
    drop(slot);
    let current = connections.reserve(Arc::new(AtomicBool::new(false))).unwrap();
    current.attach(&tcp).unwrap();
    assert!(current.current());
}

#[test]
fn retries_back_off_to_a_fixed_bound_and_success_restarts_regular_automatic_exchange() {
    let mut peer = Peer { addresses:vec![], next_address:0, next:Instant::now(), failures:0, busy:true };
    for _ in 0..100 { peer.finish(false); }
    assert_eq!(peer.failures, 5);
    assert!(peer.next.saturating_duration_since(Instant::now()) > Duration::from_secs(31));
    assert!(peer.next.saturating_duration_since(Instant::now()) <= Duration::from_secs(32));
    peer.finish(true);
    assert_eq!(peer.failures, 0);
    assert!(!peer.busy);
    assert!(peer.next.saturating_duration_since(Instant::now()) <= INTERVAL);
}

#[test]
fn an_unsigned_native_controller_never_owns_a_peer_and_stops_on_its_own_thread() {
    let path = std::env::temp_dir().join(format!("vto-runtime-idle-{}-{}", std::process::id(), crate::account::now()));
    std::fs::create_dir_all(&path).unwrap();
    let core = Arc::new(crate::pipeline::worker_tests::idle_core(&path, ""));
    let runtime = Runtime::start(&core, true).unwrap();
    let deadline = Instant::now() + Duration::from_secs(2);
    while core.sync.get(0).is_none_or(|status| status.state != ConnectionState::SignedOut) {
        assert!(Instant::now() < deadline, "the owner did not report its signed-out state");
        thread::sleep(Duration::from_millis(10));
    }
    assert!(runtime.connections.slots.lock().unwrap().is_empty());
    runtime.stop().unwrap().join().unwrap();
    assert!(runtime.stop().is_none());
    assert_eq!(core.sync.get(0).unwrap().state, ConnectionState::Stopped);
    drop(runtime); drop(core);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn a_refused_background_lease_prevents_discovery_and_releases_the_native_owner_on_shutdown() {
    #[derive(Default)]
    struct Background(Mutex<Vec<(String, bool)>>);
    impl crate::Notifier for Arc<Background> {
        fn ongoing(&self, _: &str, _: &str, _: &str, _: i64) {}
        fn ready(&self, _: &str, _: &str, _: &str) {}
        fn register_sync(&self, owner: &str) -> Result<(), String> {
            self.0.lock().unwrap().push((owner.into(), false)); Ok(())
        }
        fn sync_background(&self, owner: &str, _: u64, enabled: bool) -> Result<bool, String> {
            self.0.lock().unwrap().push((owner.into(), enabled)); Ok(false)
        }
    }
    let path = std::env::temp_dir().join(format!("vto-runtime-paused-{}-{}", std::process::id(), crate::account::now()));
    std::fs::create_dir_all(&path).unwrap();
    let mut core = crate::pipeline::worker_tests::idle_core(&path, "");
    core.account = crate::account::tests::signed_in_manager(&path, "paused-account");
    let generation = core.account.status().unwrap().generation;
    let background = Arc::new(Background::default());
    core.svc.notifier = Box::new(background.clone());
    let core = Arc::new(core);
    let runtime = Runtime::start(&core, false).unwrap();
    let deadline = Instant::now() + Duration::from_secs(2);
    while core.sync.get(generation).is_none_or(|status| status.state != ConnectionState::BackgroundPaused) {
        assert!(Instant::now() < deadline); thread::sleep(Duration::from_millis(10));
    }
    assert!(runtime.connections.slots.lock().unwrap().is_empty());
    runtime.stop().unwrap().join().unwrap();
    let calls = background.0.lock().unwrap();
    assert!(calls.iter().any(|(_, enabled)| *enabled));
    assert_eq!(calls.last().unwrap().1, false);
    assert_eq!(calls[0].0.len(), 32);
    assert!(calls.iter().all(|(owner, _)| owner == &calls[0].0));
    drop(calls); drop(runtime); drop(core);
    std::fs::remove_dir_all(path).unwrap();
}
