//! Outbound HTTP for the app: Gemini, media downloads, YouTube.
//!
//! Windows uses WinHTTP, so TLS, proxies and certificates are the OS's.
//! Elsewhere (Android) [`portable`] speaks HTTP/1.1 itself, with TLS from
//! rustls checked against Mozilla's root certificates; so does every request
//! pinned to a checked address (media), on Windows too.

use std::io::{self, Read};
use std::sync::atomic::{AtomicBool, Ordering};

static BLOCKED: AtomicBool = AtomicBool::new(false);

/// Makes every later outbound connection fail (the contract host's offline switch).
pub fn block_network(blocked: bool) {
    BLOCKED.store(blocked, Ordering::SeqCst);
}

pub fn is_blocked() -> bool {
    BLOCKED.load(Ordering::SeqCst)
}

#[derive(Debug)]
pub struct HttpError(pub String);

impl std::fmt::Display for HttpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

pub struct Request<'a> {
    pub method: &'a str,
    pub url: &'a str,
    pub headers: Vec<(String, String)>,
    pub body: Option<&'a [u8]>,
    pub timeout_ms: i32,
    /// When false a 3xx comes back as is, with its `location`, so the caller can check the target.
    pub follow_redirects: bool,
    /// Connect to this address instead of resolving the URL's host again; the
    /// URL's name still goes in `Host`, SNI and the certificate check. Applies to
    /// the first hop only (a followed redirect resolves its own host).
    pub connect_to: Option<std::net::SocketAddr>,
}

impl<'a> Request<'a> {
    pub fn get(url: &'a str) -> Self {
        Request { method: "GET", url, headers: Vec::new(), body: None, timeout_ms: 30_000, follow_redirects: true, connect_to: None }
    }
    pub fn post(url: &'a str, body: &'a [u8]) -> Self {
        Request { method: "POST", url, headers: Vec::new(), body: Some(body), timeout_ms: 120_000, follow_redirects: true, connect_to: None }
    }
    pub fn header(mut self, name: &str, value: &str) -> Self {
        self.headers.push((name.into(), value.into()));
        self
    }
}

/// An open response whose body is read as a stream.
pub struct Response {
    pub status: u16,
    pub content_type: Option<String>,
    pub content_length: Option<u64>,
    pub location: Option<String>,
    /// The address the connection really went to, when the stack reports it.
    pub remote_ip: Option<std::net::IpAddr>,
    body: Box<dyn Read + Send>,
}

impl Response {
    pub fn bytes(mut self, limit: usize) -> Result<Vec<u8>, HttpError> {
        let mut out = Vec::new();
        (&mut self.body).take(limit as u64).read_to_end(&mut out).map_err(|e| HttpError(e.to_string()))?;
        Ok(out)
    }
}

impl Read for Response {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.body.read(buf)
    }
}

pub fn send(req: Request<'_>) -> Result<Response, HttpError> {
    if is_blocked() {
        return Err(HttpError("network is blocked".into()));
    }
    let url = url::Url::parse(req.url).map_err(|e| HttpError(format!("bad url {}: {e}", req.url)))?;
    match url.scheme() {
        "http" | "https" => {}
        s => return Err(HttpError(format!("unsupported scheme {s}"))),
    }
    if req.connect_to.is_some() {
        // WinHTTP cannot connect to a chosen address and still check the
        // certificate against the name, so pinned requests always go through rustls.
        return portable::send(&url, &req);
    }
    imp::send(&url, &req)
}

#[cfg(windows)]
mod imp {
    use super::*;
    use std::ffi::c_void;

    #[link(name = "winhttp", kind = "raw-dylib")]
    extern "system" {
        fn WinHttpOpen(agent: *const u16, access: u32, proxy: *const u16, bypass: *const u16, flags: u32) -> *mut c_void;
        fn WinHttpConnect(s: *mut c_void, server: *const u16, port: u16, reserved: u32) -> *mut c_void;
        fn WinHttpOpenRequest(c: *mut c_void, verb: *const u16, object: *const u16, version: *const u16, referrer: *const u16, accept: *const *const u16, flags: u32) -> *mut c_void;
        fn WinHttpSetTimeouts(h: *mut c_void, resolve: i32, connect: i32, send: i32, receive: i32) -> i32;
        fn WinHttpSendRequest(h: *mut c_void, headers: *const u16, headers_len: u32, optional: *const c_void, optional_len: u32, total: u32, ctx: usize) -> i32;
        fn WinHttpReceiveResponse(h: *mut c_void, reserved: *mut c_void) -> i32;
        fn WinHttpQueryHeaders(h: *mut c_void, level: u32, name: *const u16, buf: *mut c_void, len: *mut u32, index: *mut u32) -> i32;
        fn WinHttpReadData(h: *mut c_void, buf: *mut c_void, n: u32, read: *mut u32) -> i32;
        fn WinHttpCloseHandle(h: *mut c_void) -> i32;
        fn WinHttpSetOption(h: *mut c_void, option: u32, buf: *const c_void, len: u32) -> i32;
        fn WinHttpQueryOption(h: *mut c_void, option: u32, buf: *mut c_void, len: *mut u32) -> i32;
        fn WinHttpAddRequestHeaders(h: *mut c_void, headers: *const u16, len: u32, modifiers: u32) -> i32;
    }
    #[link(name = "kernel32", kind = "raw-dylib")]
    extern "system" {
        fn GetLastError() -> u32;
    }

    const ACCESS_AUTOMATIC_PROXY: u32 = 4;
    const FLAG_SECURE: u32 = 0x0080_0000;
    const QUERY_STATUS_CODE: u32 = 19;
    const QUERY_CONTENT_LENGTH: u32 = 5;
    const QUERY_CONTENT_TYPE: u32 = 1;
    const QUERY_FLAG_NUMBER: u32 = 0x2000_0000;
    const QUERY_LOCATION: u32 = 33;
    const OPTION_REDIRECT_POLICY: u32 = 88;
    const REDIRECT_POLICY_NEVER: u32 = 0;
    const OPTION_CONNECTION_INFO: u32 = 93;
    const ADDREQ_ADD_OR_REPLACE: u32 = 0x2000_0000 | 0x8000_0000;

    /// WINHTTP_CONNECTION_INFO: cbSize, then two SOCKADDR_STORAGE. winhttp.h packs it
    /// to 8 on 64-bit Windows (the addresses start at 8) and to 4 on 32-bit.
    const ADDR_AT: usize = if cfg!(target_pointer_width = "64") { 8 } else { 4 };
    pub(super) const INFO_LEN: usize = ADDR_AT + 2 * 128;
    pub(super) const REMOTE_AT: usize = ADDR_AT + 128;

    /// The remote address in a WINHTTP_CONNECTION_INFO.
    pub(super) fn remote_ip(info: &[u8; INFO_LEN]) -> Option<std::net::IpAddr> {
        let sa = &info[REMOTE_AT..];
        match u16::from_le_bytes([sa[0], sa[1]]) {
            2 => Some(std::net::Ipv4Addr::new(sa[4], sa[5], sa[6], sa[7]).into()),
            23 => Some(std::net::Ipv6Addr::from(<[u8; 16]>::try_from(&sa[8..24]).unwrap()).into()),
            _ => None,
        }
    }

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    struct Handles {
        session: *mut c_void,
        connect: *mut c_void,
        request: *mut c_void,
    }
    unsafe impl Send for Handles {}
    impl Drop for Handles {
        fn drop(&mut self) {
            for h in [self.request, self.connect, self.session] {
                if !h.is_null() {
                    unsafe { WinHttpCloseHandle(h) };
                }
            }
        }
    }

    struct Body(Handles);
    impl Read for Body {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            if is_blocked() {
                return Err(io::Error::other("network is blocked"));
            }
            let mut n = 0u32;
            let ok = unsafe { WinHttpReadData(self.0.request, buf.as_mut_ptr() as *mut c_void, buf.len().min(1 << 20) as u32, &mut n) };
            if ok == 0 {
                return Err(io::Error::other(format!("WinHttpReadData failed ({})", unsafe { GetLastError() })));
            }
            Ok(n as usize)
        }
    }

    fn fail(what: &str) -> HttpError {
        HttpError(format!("{what} failed ({})", unsafe { GetLastError() }))
    }

    fn query_string(req: *mut c_void, level: u32) -> Option<String> {
        let mut buf = [0u16; 1024];
        let mut len = (buf.len() * 2) as u32;
        let ok = unsafe { WinHttpQueryHeaders(req, level, std::ptr::null(), buf.as_mut_ptr() as *mut c_void, &mut len, std::ptr::null_mut()) };
        (ok != 0).then(|| String::from_utf16_lossy(&buf[..len as usize / 2]))
    }

    pub fn send(url: &url::Url, req: &Request<'_>) -> Result<Response, HttpError> {
        let mut h = Handles { session: std::ptr::null_mut(), connect: std::ptr::null_mut(), request: std::ptr::null_mut() };
        let agent = wide("video-translate-overlay/0.1");
        h.session = unsafe { WinHttpOpen(agent.as_ptr(), ACCESS_AUTOMATIC_PROXY, std::ptr::null(), std::ptr::null(), 0) };
        if h.session.is_null() {
            return Err(fail("WinHttpOpen"));
        }
        let host = wide(url.host_str().unwrap_or(""));
        let port = url.port_or_known_default().unwrap_or(80);
        h.connect = unsafe { WinHttpConnect(h.session, host.as_ptr(), port, 0) };
        if h.connect.is_null() {
            return Err(fail("WinHttpConnect"));
        }
        let mut object = url.path().to_string();
        if let Some(q) = url.query() {
            object.push('?');
            object.push_str(q);
        }
        let verb = wide(req.method);
        let object = wide(&object);
        let flags = if url.scheme() == "https" { FLAG_SECURE } else { 0 };
        h.request = unsafe {
            WinHttpOpenRequest(h.connect, verb.as_ptr(), object.as_ptr(), std::ptr::null(), std::ptr::null(), std::ptr::null(), flags)
        };
        if h.request.is_null() {
            return Err(fail("WinHttpOpenRequest"));
        }
        let t = req.timeout_ms;
        unsafe { WinHttpSetTimeouts(h.request, t, t, t, t) };
        if !req.follow_redirects {
            let never = REDIRECT_POLICY_NEVER;
            if unsafe { WinHttpSetOption(h.request, OPTION_REDIRECT_POLICY, &never as *const u32 as *const c_void, 4) } == 0 {
                return Err(fail("WinHttpSetOption(redirect policy)"));
            }
        }
        // Added one by one with replace-or-add, so `Host` replaces WinHTTP's own instead of doubling it.
        for (k, v) in &req.headers {
            let line: Vec<u16> = format!("{k}: {v}").encode_utf16().collect();
            if unsafe { WinHttpAddRequestHeaders(h.request, line.as_ptr(), line.len() as u32, ADDREQ_ADD_OR_REPLACE) } == 0 {
                return Err(fail("WinHttpAddRequestHeaders"));
            }
        }
        let body = req.body.unwrap_or(&[]);
        let ok = unsafe {
            WinHttpSendRequest(
                h.request,
                std::ptr::null(),
                0,
                if body.is_empty() { std::ptr::null() } else { body.as_ptr() as *const c_void },
                body.len() as u32,
                body.len() as u32,
                0,
            )
        };
        if ok == 0 {
            return Err(fail("WinHttpSendRequest"));
        }
        if unsafe { WinHttpReceiveResponse(h.request, std::ptr::null_mut()) } == 0 {
            return Err(fail("WinHttpReceiveResponse"));
        }
        let mut status = 0u32;
        let mut len = 4u32;
        unsafe {
            WinHttpQueryHeaders(
                h.request,
                QUERY_STATUS_CODE | QUERY_FLAG_NUMBER,
                std::ptr::null(),
                &mut status as *mut u32 as *mut c_void,
                &mut len,
                std::ptr::null_mut(),
            )
        };
        let content_type = query_string(h.request, QUERY_CONTENT_TYPE);
        let content_length = query_string(h.request, QUERY_CONTENT_LENGTH).and_then(|s| s.trim().parse().ok());
        let location = query_string(h.request, QUERY_LOCATION);
        let mut info = [0u8; INFO_LEN];
        info[..4].copy_from_slice(&(INFO_LEN as u32).to_le_bytes());
        let mut len = info.len() as u32;
        let ok = unsafe { WinHttpQueryOption(h.request, OPTION_CONNECTION_INFO, info.as_mut_ptr() as *mut c_void, &mut len) };
        let remote_ip = if ok != 0 { remote_ip(&info) } else { None };
        Ok(Response { status: status as u16, content_type, content_length, location, remote_ip, body: Box::new(Body(h)) })
    }
}

#[cfg(not(windows))]
use portable as imp;

/// HTTP/1.1 over TCP, or over rustls for `https`: every request off Windows,
/// and pinned ones on Windows.
pub(crate) mod portable {
    use super::*;
    use std::io::{BufRead, BufReader, Write};
    use std::net::{TcpStream, ToSocketAddrs};
    use std::sync::{Arc, OnceLock};
    use std::time::Duration;

    trait Conn: Read + Write + Send {}
    impl<T: Read + Write + Send> Conn for T {}

    fn tls() -> Result<Arc<rustls::ClientConfig>, HttpError> {
        static CONFIG: OnceLock<Arc<rustls::ClientConfig>> = OnceLock::new();
        if let Some(c) = CONFIG.get() {
            return Ok(c.clone());
        }
        let roots = rustls::RootCertStore { roots: webpki_roots::TLS_SERVER_ROOTS.to_vec() };
        let c = rustls::ClientConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
            .with_safe_default_protocol_versions()
            .map_err(|e| HttpError(e.to_string()))?
            .with_root_certificates(roots)
            .with_no_client_auth();
        Ok(CONFIG.get_or_init(|| Arc::new(c)).clone())
    }

    /// Follows redirects (up to five) when the request asks for it.
    pub fn send(url: &url::Url, req: &Request<'_>) -> Result<Response, HttpError> {
        let mut url = url.clone();
        let mut pin = req.connect_to;
        for _ in 0..5 {
            let r = once(&url, req, pin)?;
            match (&r.location, req.follow_redirects && (300..400).contains(&r.status)) {
                (Some(to), true) => {
                    url = url.join(to).map_err(|e| HttpError(format!("bad redirect {to}: {e}")))?;
                    pin = None;
                }
                _ => return Ok(r),
            }
        }
        Err(HttpError("too many redirects".into()))
    }

    fn once(url: &url::Url, req: &Request<'_>, pin: Option<std::net::SocketAddr>) -> Result<Response, HttpError> {
        let host = url.host_str().unwrap_or("").trim_start_matches('[').trim_end_matches(']');
        let port = url.port_or_known_default().unwrap_or(80);
        let err = |e: io::Error| HttpError(e.to_string());
        let timeout = Duration::from_millis(req.timeout_ms.max(1) as u64);
        let addrs: Vec<_> = match pin {
            Some(a) => vec![a],
            None => (host, port).to_socket_addrs().map_err(err)?.collect(),
        };
        let mut last = HttpError(format!("{host} has no address"));
        let mut tcp = None;
        for a in addrs {
            match TcpStream::connect_timeout(&a, timeout) {
                Ok(s) => {
                    tcp = Some(s);
                    break;
                }
                Err(e) => last = err(e),
            }
        }
        let tcp = tcp.ok_or(last)?;
        let remote_ip = tcp.peer_addr().ok().map(|a| a.ip());
        tcp.set_read_timeout(Some(timeout)).map_err(err)?;
        tcp.set_write_timeout(Some(timeout)).map_err(err)?;
        let mut s: Box<dyn Conn> = if url.scheme() == "https" {
            let name = rustls::pki_types::ServerName::try_from(host.to_string()).map_err(|e| HttpError(e.to_string()))?;
            let c = rustls::ClientConnection::new(tls()?, name).map_err(|e| HttpError(e.to_string()))?;
            Box::new(rustls::StreamOwned::new(c, tcp))
        } else {
            Box::new(tcp)
        };
        let mut path = url.path().to_string();
        if let Some(q) = url.query() {
            path.push('?');
            path.push_str(q);
        }
        let body = req.body.unwrap_or(&[]);
        let mut head = format!("{} {path} HTTP/1.1\r\nConnection: close\r\nContent-Length: {}\r\n", req.method, body.len());
        if !req.headers.iter().any(|(k, _)| k.eq_ignore_ascii_case("host")) {
            // The URL's own authority: brackets around IPv6, the port only when it is not the default.
            let authority = &url[url::Position::BeforeHost..url::Position::AfterPort];
            head.push_str(&format!("Host: {authority}\r\n"));
        }
        for (k, v) in &req.headers {
            head.push_str(&format!("{k}: {v}\r\n"));
        }
        head.push_str("\r\n");
        s.write_all(head.as_bytes()).map_err(err)?;
        s.write_all(body).map_err(err)?;
        let mut r = BufReader::new(s);
        let mut line = String::new();
        r.read_line(&mut line).map_err(err)?;
        let status = line.split_whitespace().nth(1).and_then(|c| c.parse().ok()).unwrap_or(0);
        // Plain TCP never follows redirects; callers that want them get `location` back.
        let (mut content_type, mut content_length, mut chunked, mut location) = (None, None, false, None);
        loop {
            let mut h = String::new();
            if r.read_line(&mut h).map_err(err)? == 0 || h.trim().is_empty() {
                break;
            }
            if let Some((k, v)) = h.split_once(':') {
                let (k, v) = (k.trim().to_ascii_lowercase(), v.trim().to_string());
                match k.as_str() {
                    "content-type" => content_type = Some(v),
                    "content-length" => content_length = v.parse().ok(),
                    "transfer-encoding" => chunked = v.to_ascii_lowercase().contains("chunked"),
                    "location" => location = Some(v),
                    _ => {}
                }
            }
        }
        let body: Box<dyn Read + Send> = if chunked {
            Box::new(Chunked { r, left: 0, done: false })
        } else if let Some(n) = content_length {
            Box::new(r.take(n))
        } else {
            Box::new(r)
        };
        Ok(Response { status, content_type, content_length, location, remote_ip, body })
    }

    /// A chunked body, streamed: the server's chunk sizes only bound how much is
    /// read next, so no size it sends can make this allocate. A size line that
    /// is not hex, or one past 16 hex digits, is an error, not the end.
    pub(super) struct Chunked<R> {
        pub(super) r: R,
        pub(super) left: u64,
        pub(super) done: bool,
    }

    impl<R: BufRead> Chunked<R> {
        /// A line of at most 1 KiB, without its CRLF.
        fn line(&mut self) -> io::Result<String> {
            let mut l = Vec::new();
            (&mut self.r).take(1024).read_until(b'\n', &mut l)?;
            if l.last() != Some(&b'\n') {
                return Err(io::Error::new(io::ErrorKind::InvalidData, "chunked: line cut short or too long"));
            }
            Ok(String::from_utf8_lossy(&l).trim_end_matches(['\r', '\n']).to_string())
        }
    }

    impl<R: BufRead> Read for Chunked<R> {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            let bad = |m: &str| io::Error::new(io::ErrorKind::InvalidData, format!("chunked: {m}"));
            if self.done || buf.is_empty() {
                return Ok(0);
            }
            if self.left == 0 {
                let size = self.line()?;
                let hex = size.split(';').next().unwrap_or("").trim();
                self.left = u64::from_str_radix(hex, 16).map_err(|_| bad(&format!("bad size {hex:?}")))?;
                if self.left == 0 {
                    // Trailer fields up to the blank line.
                    while !self.line()?.is_empty() {}
                    self.done = true;
                    return Ok(0);
                }
            }
            let want = buf.len().min(usize::try_from(self.left).unwrap_or(usize::MAX));
            let n = self.r.read(&mut buf[..want])?;
            if n == 0 {
                return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "chunked: body cut short"));
            }
            self.left -= n as u64;
            if self.left == 0 && !self.line()?.is_empty() {
                return Err(bad("no CRLF after a chunk"));
            }
            Ok(n)
        }
    }
}

#[cfg(test)]
#[path = "http_portable_tests.rs"]
mod portable_tests;

#[cfg(test)]
#[path = "http_chunked_tests.rs"]
mod chunked_tests;

#[cfg(test)]
#[path = "http_pin_tests.rs"]
mod pin_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader, Write};

    #[test]
    fn a_pinned_request_carries_its_host_name_and_reports_the_peer() {
        let server = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = server.local_addr().unwrap().port();
        let seen = std::thread::spawn(move || {
            let (s, _) = server.accept().unwrap();
            let mut r = BufReader::new(s.try_clone().unwrap());
            let mut head = Vec::new();
            loop {
                let mut l = String::new();
                if r.read_line(&mut l).unwrap() == 0 || l.trim().is_empty() {
                    break;
                }
                head.push(l.trim().to_string());
            }
            (&s).write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok").unwrap();
            head
        });
        let url = format!("http://127.0.0.1:{port}/a.m4a");
        let r = send(Request::get(&url).header("Host", "media.example.org:8080")).unwrap();
        assert_eq!(r.status, 200);
        assert_eq!(r.remote_ip, Some(std::net::Ipv4Addr::LOCALHOST.into()));
        let head = seen.join().unwrap();
        let hosts: Vec<&String> = head.iter().filter(|l| l.to_ascii_lowercase().starts_with("host:")).collect();
        assert_eq!(hosts, [&"Host: media.example.org:8080".to_string()], "{head:?}");
    }

    #[cfg(windows)]
    #[test]
    fn connection_info_addresses_are_read() {
        let at = imp::REMOTE_AT;
        let mut info = [0u8; imp::INFO_LEN];
        info[at..at + 2].copy_from_slice(&2u16.to_le_bytes());
        info[at + 4..at + 8].copy_from_slice(&[10, 0, 0, 7]);
        assert_eq!(imp::remote_ip(&info), Some(std::net::Ipv4Addr::new(10, 0, 0, 7).into()));
        info[at..at + 2].copy_from_slice(&23u16.to_le_bytes());
        info[at + 8..at + 24].copy_from_slice(&std::net::Ipv6Addr::LOCALHOST.octets());
        assert_eq!(imp::remote_ip(&info), Some(std::net::Ipv6Addr::LOCALHOST.into()));
    }
}
