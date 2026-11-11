//! Outbound HTTP for the app: Gemini, media downloads, YouTube.
//!
//! Windows uses WinHTTP, so TLS, proxies and certificates are the OS's.
//! Elsewhere only plain `http://` is available until a platform stack lands.

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
}

impl<'a> Request<'a> {
    pub fn get(url: &'a str) -> Self {
        Request { method: "GET", url, headers: Vec::new(), body: None, timeout_ms: 30_000 }
    }
    pub fn post(url: &'a str, body: &'a [u8]) -> Self {
        Request { method: "POST", url, headers: Vec::new(), body: Some(body), timeout_ms: 120_000 }
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
        let headers: String = req.headers.iter().map(|(k, v)| format!("{k}: {v}\r\n")).collect();
        let headers_w: Vec<u16> = headers.encode_utf16().collect();
        let body = req.body.unwrap_or(&[]);
        let ok = unsafe {
            WinHttpSendRequest(
                h.request,
                if headers_w.is_empty() { std::ptr::null() } else { headers_w.as_ptr() },
                headers_w.len() as u32,
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
        Ok(Response { status: status as u16, content_type, content_length, body: Box::new(Body(h)) })
    }
}

#[cfg(not(windows))]
mod imp {
    use super::*;
    use std::io::{BufRead, BufReader, Write};
    use std::net::TcpStream;
    use std::time::Duration;

    pub fn send(url: &url::Url, req: &Request<'_>) -> Result<Response, HttpError> {
        if url.scheme() != "http" {
            return Err(HttpError("https needs the platform HTTP stack, not available on this OS yet".into()));
        }
        let host = url.host_str().unwrap_or("");
        let port = url.port_or_known_default().unwrap_or(80);
        let err = |e: io::Error| HttpError(e.to_string());
        let mut s = TcpStream::connect((host, port)).map_err(err)?;
        s.set_read_timeout(Some(Duration::from_millis(req.timeout_ms as u64))).map_err(err)?;
        let mut path = url.path().to_string();
        if let Some(q) = url.query() {
            path.push('?');
            path.push_str(q);
        }
        let body = req.body.unwrap_or(&[]);
        let mut head = format!("{} {path} HTTP/1.1\r\nHost: {host}:{port}\r\nConnection: close\r\nContent-Length: {}\r\n", req.method, body.len());
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
        let (mut content_type, mut content_length, mut chunked) = (None, None, false);
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
                    _ => {}
                }
            }
        }
        let body: Box<dyn Read + Send> = if chunked {
            let mut all = Vec::new();
            loop {
                let mut size = String::new();
                r.read_line(&mut size).map_err(err)?;
                let n = usize::from_str_radix(size.trim().split(';').next().unwrap_or("0"), 16).unwrap_or(0);
                let mut chunk = vec![0; n + 2];
                r.read_exact(&mut chunk).map_err(err)?;
                if n == 0 {
                    break;
                }
                all.extend_from_slice(&chunk[..n]);
            }
            Box::new(io::Cursor::new(all))
        } else if let Some(n) = content_length {
            Box::new(r.take(n))
        } else {
            Box::new(r)
        };
        Ok(Response { status, content_type, content_length, body })
    }
}
