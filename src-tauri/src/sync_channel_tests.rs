use super::*;
use crate::{
    account::NativeAccount,
    google_auth::{
        tests::{claims, signed},
        Verifier,
    },
    google_desktop::Credentials,
    sync::{Change, Cue, Deletions, Translation, Video},
};
use std::{
    net::TcpListener,
    path::PathBuf,
    sync::{mpsc, Mutex},
    thread,
};

struct Profile(PathBuf);
impl Profile {
    fn new() -> Self {
        let mut random = [0u8; 16];
        getrandom::fill(&mut random).unwrap();
        let path = std::env::temp_dir().join(format!(
            "vto-peer-{}",
            random
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<String>()
        ));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
    fn manager(&self, subject: &str, platform: Platform) -> Arc<Manager> {
        let manager = Arc::new(Manager::with_test_verifier(self.0.clone()));
        manager
            .configure(Arc::new(Native {
                subject: subject.into(),
                platform,
                saved: Mutex::new(None),
            }))
            .unwrap();
        manager.restore().unwrap();
        manager
            .sign_in(manager.status().unwrap().generation)
            .unwrap();
        manager
    }
    fn historical_manager(&self, subject: &str, platform: Platform) -> Arc<Manager> {
        let manager = Arc::new(Manager::new(self.0.clone()));
        manager
            .configure(Arc::new(Native {
                subject: subject.into(),
                platform,
                saved: Mutex::new(Some(crate::account::tests::historical_receipt(
                    subject,
                    platform,
                    crate::account::now() - 7200,
                ))),
            }))
            .unwrap();
        assert_eq!(
            manager.restore().unwrap().state,
            crate::account::AuthState::SignedIn
        );
        manager
    }
}
impl Drop for Profile {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}
struct Native {
    subject: String,
    platform: Platform,
    saved: Mutex<Option<String>>,
}
impl NativeAccount for Native {
    fn platform(&self) -> Platform {
        self.platform
    }
    fn read(&self) -> std::result::Result<Option<String>, AuthError> {
        Ok(self.saved.lock().unwrap().clone())
    }
    fn write(&self, value: &str) -> std::result::Result<(), AuthError> {
        *self.saved.lock().unwrap() = Some(value.into());
        Ok(())
    }
    fn delete(&self) -> std::result::Result<(), AuthError> {
        *self.saved.lock().unwrap() = None;
        Ok(())
    }
    fn sign_in(
        &self,
        nonce: &str,
        _: &AtomicBool,
        verifier: &Verifier,
    ) -> std::result::Result<Credentials, AuthError> {
        let mut value = claims(self.platform);
        value["sub"] = self.subject.clone().into();
        value["nonce"] = nonce.into();
        let token = signed(value);
        Ok(Credentials {
            identity: verifier.verify(&token, nonce, self.platform)?,
            id_token: token,
            refresh_token: None,
        })
    }
}
fn translation() -> Change {
    let page = "https://lectures.example.org/direct-peer";
    Change::Translation {
        translation: Translation {
            video: Video {
                key: crate::api::video_key(&url::Url::parse(page).unwrap()),
                page_url: page.into(),
                title: "Direct local transfer".into(),
            },
            duration_ms: 5000,
            source_language: Some("en".into()),
            terms: vec![],
            model: crate::gemini::MODEL.into(),
            cost_krw: 12.5,
            cues: vec![Cue {
                start_ms: 1234,
                end_ms: 4321,
                source_text: "Complete original".into(),
                text_ko: "Complete translated response".into(),
            }],
        },
    }
}

#[test]
fn real_tcp_tls_connections_transfer_completed_data_both_ways_and_converge_without_echo() {
    let a = Profile::new();
    let b = Profile::new();
    let pc = a.manager("same-account", Platform::Desktop);
    let phone = b.manager("same-account", Platform::Android);
    let pc_session = pc.session().unwrap();
    let phone_session = phone.session().unwrap();
    pc.with_session(&pc_session, |journal| {
        journal.write(translation(), Deletions::new()).map(drop)
    })
    .unwrap();
    phone_session
        .journal
        .library()
        .unwrap()
        .create_playlist("phone-topic", "Created on phone")
        .unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let owner = pc.clone();
    let server = thread::spawn(move || {
        let (tcp, _) = listener.accept().unwrap();
        let mut channel = Channel::server(tcp, owner.session().unwrap(), &owner).unwrap();
        (0..4)
            .map(|_| channel.exchange(&owner).unwrap())
            .collect::<Vec<_>>()
    });
    let mut channel = Channel::client(
        TcpStream::connect(address).unwrap(),
        phone_session.clone(),
        &phone,
    )
    .unwrap();
    assert!(channel.exchange(&phone).unwrap());
    let store = phone_session.journal.library().unwrap();
    let Change::Translation { translation: t } = translation() else {
        unreachable!()
    };
    assert_eq!(
        store.ready_job(&t.video.key).unwrap().unwrap().cost_krw,
        12.5
    );
    assert_eq!(
        (
            store.cues(&t.video.key).unwrap()[0].start_ms,
            store.cues(&t.video.key).unwrap()[0].end_ms
        ),
        (1234, 4321)
    );
    assert!(store.active_job().unwrap().is_none());
    store
        .add_entry(
            "phone-topic",
            &t.video.key,
            &t.video.page_url,
            "other",
            &t.video.title,
        )
        .unwrap();
    channel.exchange(&phone).unwrap();
    channel.exchange(&phone).unwrap();
    assert!(!channel.exchange(&phone).unwrap());
    let changed = server.join().unwrap();
    assert_eq!(&changed[2..], &[false, false]);
    assert_eq!(
        store.library().unwrap(),
        pc_session.journal.library().unwrap().library().unwrap()
    );
    drop(channel);
    drop(store);
    drop(pc_session);
    drop(phone_session);
    drop(pc);
    drop(phone);
    let pc_journal = crate::sync::Journal::open(
        &a.0,
        &crate::google_auth::VerifiedIdentity::fixture("same-account"),
    )
    .unwrap();
    let phone_journal = crate::sync::Journal::open(
        &b.0,
        &crate::google_auth::VerifiedIdentity::fixture("same-account"),
    )
    .unwrap();
    assert_eq!(
        pc_journal.library().unwrap().library().unwrap(),
        phone_journal.library().unwrap().library().unwrap()
    );
    assert_eq!(
        phone_journal.library().unwrap().library().unwrap()["playlists"][0]["entries"][0],
        t.video.key
    );
}

#[test]
fn different_signed_google_subjects_with_the_same_email_cannot_transfer_any_library_data() {
    let a = Profile::new();
    let b = Profile::new();
    let pc = a.manager("account-one", Platform::Desktop);
    let phone = b.manager("account-two", Platform::Android);
    let session = pc.session().unwrap();
    session
        .journal
        .write(translation(), Deletions::new())
        .unwrap();
    let original = session.journal.records().unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let owner = pc.clone();
    let server = thread::spawn(move || {
        let (tcp, _) = listener.accept().unwrap();
        assert!(Channel::server(tcp, owner.session().unwrap(), &owner).is_err());
    });
    assert!(matches!(
        Channel::client(
            TcpStream::connect(address).unwrap(),
            phone.session().unwrap(),
            &phone
        ),
        Err(Failure::Auth(AuthError::AccountMismatch))
    ));
    server.join().unwrap();
    assert_eq!(session.journal.records().unwrap(), original);
    assert!(phone
        .session()
        .unwrap()
        .journal
        .records()
        .unwrap()
        .is_empty());
}

#[test]
fn sign_out_cancels_an_authenticated_connection_waiting_for_the_peer() {
    let a = Profile::new();
    let b = Profile::new();
    let pc = a.manager("same-account", Platform::Desktop);
    let phone = b.manager("same-account", Platform::Android);
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let (ready, armed) = mpsc::channel();
    let (done, completed) = mpsc::channel();
    let owner = pc.clone();
    let server = thread::spawn(move || {
        let (tcp, _) = listener.accept().unwrap();
        let mut channel = Channel::server(tcp, owner.session().unwrap(), &owner).unwrap();
        ready.send(()).unwrap();
        done.send(channel.exchange(&owner)).unwrap();
    });
    let _idle = Channel::client(
        TcpStream::connect(address).unwrap(),
        phone.session().unwrap(),
        &phone,
    )
    .unwrap();
    armed.recv_timeout(Duration::from_secs(5)).unwrap();
    pc.sign_out(pc.status().unwrap().generation).unwrap();
    assert!(matches!(
        completed.recv_timeout(Duration::from_secs(2)).unwrap(),
        Err(Failure::Auth(AuthError::Cancelled))
    ));
    server.join().unwrap();
}

#[test]
fn disconnect_after_one_committed_direction_reconnects_from_durable_cursors() {
    let a = Profile::new();
    let b = Profile::new();
    let pc = a.manager("same-account", Platform::Desktop);
    let phone = b.manager("same-account", Platform::Android);
    let pc_session = pc.session().unwrap();
    let phone_session = phone.session().unwrap();
    pc_session
        .journal
        .write(translation(), Deletions::new())
        .unwrap();
    phone_session
        .journal
        .library()
        .unwrap()
        .create_playlist("waiting", "Unsent phone change")
        .unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let owner = pc.clone();
    let server = thread::spawn(move || {
        let (tcp, _) = listener.accept().unwrap();
        let mut partial = Channel::server(tcp, owner.session().unwrap(), &owner).unwrap();
        assert!(partial.exchange(&owner).is_err());
        drop(partial);
        let (tcp, _) = listener.accept().unwrap();
        let mut retried = Channel::server(tcp, owner.session().unwrap(), &owner).unwrap();
        retried.exchange(&owner).unwrap();
    });
    let mut partial = Channel::client(
        TcpStream::connect(address).unwrap(),
        phone_session.clone(),
        &phone,
    )
    .unwrap();
    partial.write(Frame::Cursor { through: 0 }).unwrap();
    let incoming = partial.page().unwrap();
    assert!(partial.receive(&phone, &incoming).unwrap());
    drop(partial);
    assert!(
        pc_session.journal.library().unwrap().library().unwrap()["playlists"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    let reopened = crate::sync::Journal::open(
        &b.0,
        &crate::google_auth::VerifiedIdentity::fixture("same-account"),
    )
    .unwrap();
    assert_eq!(
        reopened.cursor(pc_session.journal.replica()).unwrap(),
        incoming.through
    );
    let mut retried = Channel::client(
        TcpStream::connect(address).unwrap(),
        phone_session.clone(),
        &phone,
    )
    .unwrap();
    retried.exchange(&phone).unwrap();
    server.join().unwrap();
    assert_eq!(
        pc_session.journal.library().unwrap().library().unwrap(),
        phone_session.journal.library().unwrap().library().unwrap()
    );
}

#[test]
fn framing_rejects_oversized_lengths_unknown_operations_fields_and_truncated_pages() {
    for bytes in [0u32.to_be_bytes(), (CONTROL_LIMIT as u32 + 1).to_be_bytes()] {
        assert!(matches!(
            read(&mut &bytes[..], CONTROL_LIMIT),
            Err(Failure::Protocol)
        ));
    }
    for body in [
        r#"{"type":"read_file","path":"C:/private"}"#,
        r#"{"type":"cursor","through":0,"sql":"SELECT secret"}"#,
        r#"{"type":"proof","token":"x","platform":"other","replica":"x"}"#,
    ] {
        let mut encoded = (body.len() as u32).to_be_bytes().to_vec();
        encoded.extend(body.as_bytes());
        assert!(matches!(
            read(&mut &encoded[..], CONTROL_LIMIT),
            Err(Failure::Protocol)
        ));
    }
    assert!(matches!(
        read(&mut &8u32.to_be_bytes()[..], PAGE_LIMIT),
        Err(Failure::Transport)
    ));
    let mut output = Vec::new();
    assert!(matches!(
        write(
            &mut output,
            &Frame::Proof {
                token: Some("x".repeat(8193)),
                platform: Platform::Desktop,
                replica: "a".repeat(32)
            }
        ),
        Err(Failure::Protocol)
    ));
    assert!(output.is_empty());
}

#[test]
fn enrolled_devices_reconnect_with_expired_google_proofs_but_expired_peer_receipts_require_google_again(
) {
    let a = Profile::new();
    let b = Profile::new();
    // These native receipts and peer rows model a pair enrolled two hours earlier.
    let pc = a.historical_manager("same-account", Platform::Desktop);
    let phone = b.historical_manager("same-account", Platform::Android);
    let pc_session = pc.session().unwrap();
    let phone_session = phone.session().unwrap();
    assert!(matches!(pc_session.peer_proof(), Err(AuthError::Expired)));
    assert!(matches!(
        phone_session.peer_proof(),
        Err(AuthError::Expired)
    ));
    pc_session
        .journal
        .trust_peer(
            &phone_session.device().unwrap().nonce(),
            phone_session.identity.expires_at(),
        )
        .unwrap();
    phone_session
        .journal
        .trust_peer(
            &pc_session.device().unwrap().nonce(),
            pc_session.identity.expires_at(),
        )
        .unwrap();
    pc_session
        .journal
        .write(translation(), Deletions::new())
        .unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let owner = pc.clone();
    let server = thread::spawn(move || {
        let (tcp, _) = listener.accept().unwrap();
        let mut channel = Channel::server(tcp, owner.session().unwrap(), &owner).unwrap();
        channel.exchange(&owner).unwrap();
        drop(channel);
        let (tcp, _) = listener.accept().unwrap();
        assert!(matches!(
            Channel::server(tcp, owner.session().unwrap(), &owner),
            Err(Failure::Auth(AuthError::Expired))
        ));
    });
    let mut channel = Channel::client(
        TcpStream::connect(address).unwrap(),
        phone_session.clone(),
        &phone,
    )
    .unwrap();
    assert!(channel.exchange(&phone).unwrap());
    channel.authorized_until = Some(crate::account::now() - 1);
    assert!(matches!(
        channel.exchange(&phone),
        Err(Failure::Auth(AuthError::Expired))
    ));
    drop(channel);
    let store = phone_session.journal.library().unwrap();
    let db = crate::db::Db::open(store.path()).unwrap();
    db.execute(
        "UPDATE sync_peer SET expires_at=?",
        crate::p![crate::account::now() - 1],
    )
    .unwrap();
    assert!(matches!(
        Channel::client(
            TcpStream::connect(address).unwrap(),
            phone_session.clone(),
            &phone
        ),
        Err(Failure::Auth(AuthError::Expired))
    ));
    server.join().unwrap();
    assert_eq!(store.library().unwrap()["videos"][0]["saved"], true);
}

#[test]
fn an_unknown_certificate_cannot_enroll_using_an_expired_google_proof() {
    let a = Profile::new();
    let b = Profile::new();
    let pc = a.historical_manager("same-account", Platform::Desktop);
    let phone = b.historical_manager("same-account", Platform::Android);
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let owner = pc.clone();
    let server = thread::spawn(move || {
        let (tcp, _) = listener.accept().unwrap();
        assert!(matches!(
            Channel::server(tcp, owner.session().unwrap(), &owner),
            Err(Failure::Auth(AuthError::Expired))
        ));
    });
    assert!(matches!(
        Channel::client(
            TcpStream::connect(address).unwrap(),
            phone.session().unwrap(),
            &phone
        ),
        Err(Failure::Auth(AuthError::Expired))
    ));
    server.join().unwrap();
    assert!(pc.session().unwrap().journal.records().unwrap().is_empty());
    assert!(phone
        .session()
        .unwrap()
        .journal
        .records()
        .unwrap()
        .is_empty());
}
