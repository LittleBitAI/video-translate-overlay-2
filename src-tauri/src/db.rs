//! A small binding to the SQLite the OS ships: `winsqlite3.dll` on Windows,
//! the system `libsqlite3` elsewhere. Only what the store needs.

use std::ffi::{c_char, c_int, c_void, CStr, CString};
use std::path::Path;

#[cfg_attr(windows, link(name = "winsqlite3", kind = "raw-dylib"))]
#[cfg_attr(not(windows), link(name = "sqlite3"))]
extern "C" {
    fn sqlite3_open_v2(filename: *const c_char, db: *mut *mut c_void, flags: c_int, vfs: *const c_char) -> c_int;
    fn sqlite3_close_v2(db: *mut c_void) -> c_int;
    fn sqlite3_errmsg(db: *mut c_void) -> *const c_char;
    fn sqlite3_busy_timeout(db: *mut c_void, ms: c_int) -> c_int;
    fn sqlite3_prepare_v2(db: *mut c_void, sql: *const c_char, n: c_int, stmt: *mut *mut c_void, tail: *mut *const c_char) -> c_int;
    fn sqlite3_bind_int64(stmt: *mut c_void, i: c_int, v: i64) -> c_int;
    fn sqlite3_bind_double(stmt: *mut c_void, i: c_int, v: f64) -> c_int;
    fn sqlite3_bind_text(stmt: *mut c_void, i: c_int, v: *const c_char, n: c_int, destructor: isize) -> c_int;
    fn sqlite3_bind_null(stmt: *mut c_void, i: c_int) -> c_int;
    fn sqlite3_step(stmt: *mut c_void) -> c_int;
    fn sqlite3_column_count(stmt: *mut c_void) -> c_int;
    fn sqlite3_column_type(stmt: *mut c_void, i: c_int) -> c_int;
    fn sqlite3_column_int64(stmt: *mut c_void, i: c_int) -> i64;
    fn sqlite3_column_double(stmt: *mut c_void, i: c_int) -> f64;
    fn sqlite3_column_text(stmt: *mut c_void, i: c_int) -> *const u8;
    fn sqlite3_column_bytes(stmt: *mut c_void, i: c_int) -> c_int;
    fn sqlite3_finalize(stmt: *mut c_void) -> c_int;
    fn sqlite3_changes(db: *mut c_void) -> c_int;
}

const OK: c_int = 0;
const ROW: c_int = 100;
const DONE: c_int = 101;
const OPEN_READWRITE: c_int = 0x2;
const OPEN_CREATE: c_int = 0x4;
const OPEN_FULLMUTEX: c_int = 0x10000;
const TRANSIENT: isize = -1;

#[derive(Debug, Clone, PartialEq)]
pub enum Val {
    Null,
    Int(i64),
    Real(f64),
    Text(String),
}

impl Val {
    pub fn int(&self) -> i64 {
        match self {
            Val::Int(i) => *i,
            Val::Real(r) => *r as i64,
            _ => 0,
        }
    }
    pub fn real(&self) -> f64 {
        match self {
            Val::Real(r) => *r,
            Val::Int(i) => *i as f64,
            _ => 0.0,
        }
    }
    pub fn text(&self) -> String {
        self.opt_text().unwrap_or_default()
    }
    pub fn opt_text(&self) -> Option<String> {
        match self {
            Val::Text(s) => Some(s.clone()),
            _ => None,
        }
    }
}

impl From<i64> for Val {
    fn from(v: i64) -> Self {
        Val::Int(v)
    }
}
impl From<f64> for Val {
    fn from(v: f64) -> Self {
        Val::Real(v)
    }
}
impl From<&str> for Val {
    fn from(v: &str) -> Self {
        Val::Text(v.into())
    }
}
impl From<String> for Val {
    fn from(v: String) -> Self {
        Val::Text(v)
    }
}
impl From<&String> for Val {
    fn from(v: &String) -> Self {
        Val::Text(v.clone())
    }
}
impl<T: Into<Val>> From<Option<T>> for Val {
    fn from(v: Option<T>) -> Self {
        v.map(Into::into).unwrap_or(Val::Null)
    }
}

/// Builds a parameter slice: `p![a, b]`.
#[macro_export]
macro_rules! p {
    ($($v:expr),* $(,)?) => { &[$($crate::db::Val::from($v)),*][..] };
}

pub struct Db {
    raw: *mut c_void,
}

// The connection is opened in serialized (FULLMUTEX) mode.
unsafe impl Send for Db {}
unsafe impl Sync for Db {}

pub type Row = Vec<Val>;

impl Db {
    pub fn open(path: &Path) -> Result<Db, String> {
        let c = CString::new(path.to_string_lossy().as_bytes()).map_err(|e| e.to_string())?;
        let mut raw = std::ptr::null_mut();
        let rc = unsafe { sqlite3_open_v2(c.as_ptr(), &mut raw, OPEN_READWRITE | OPEN_CREATE | OPEN_FULLMUTEX, std::ptr::null()) };
        let db = Db { raw };
        if rc != OK {
            return Err(format!("open {}: {}", path.display(), db.err()));
        }
        unsafe { sqlite3_busy_timeout(raw, 5_000) };
        Ok(db)
    }

    fn err(&self) -> String {
        if self.raw.is_null() {
            return "out of memory".into();
        }
        unsafe { CStr::from_ptr(sqlite3_errmsg(self.raw)) }.to_string_lossy().into_owned()
    }

    /// Runs `;`-separated statements without parameters.
    pub fn exec(&self, sql: &str) -> Result<(), String> {
        for stmt in sql.split(';').map(str::trim).filter(|s| !s.is_empty()) {
            self.execute(stmt, &[])?;
        }
        Ok(())
    }

    pub fn execute(&self, sql: &str, params: &[Val]) -> Result<usize, String> {
        self.run(sql, params, |_| ())?;
        Ok(unsafe { sqlite3_changes(self.raw) } as usize)
    }

    pub fn query(&self, sql: &str, params: &[Val]) -> Result<Vec<Row>, String> {
        let mut rows = Vec::new();
        self.run(sql, params, |r| rows.push(r))?;
        Ok(rows)
    }

    pub fn query_one(&self, sql: &str, params: &[Val]) -> Result<Option<Row>, String> {
        Ok(self.query(sql, params)?.into_iter().next())
    }

    fn run(&self, sql: &str, params: &[Val], mut on_row: impl FnMut(Row)) -> Result<(), String> {
        let c = CString::new(sql).map_err(|e| e.to_string())?;
        let mut stmt = std::ptr::null_mut();
        let rc = unsafe { sqlite3_prepare_v2(self.raw, c.as_ptr(), -1, &mut stmt, std::ptr::null_mut()) };
        if rc != OK {
            return Err(format!("{}: {sql}", self.err()));
        }
        let result = (|| {
            let mut texts = Vec::new();
            for (i, p) in params.iter().enumerate() {
                let i = i as c_int + 1;
                let rc = unsafe {
                    match p {
                        Val::Null => sqlite3_bind_null(stmt, i),
                        Val::Int(v) => sqlite3_bind_int64(stmt, i, *v),
                        Val::Real(v) => sqlite3_bind_double(stmt, i, *v),
                        Val::Text(s) => {
                            texts.push(s.as_bytes().to_vec());
                            let b = texts.last().unwrap();
                            sqlite3_bind_text(stmt, i, b.as_ptr() as *const c_char, b.len() as c_int, TRANSIENT)
                        }
                    }
                };
                if rc != OK {
                    return Err(self.err());
                }
            }
            loop {
                match unsafe { sqlite3_step(stmt) } {
                    ROW => {
                        let n = unsafe { sqlite3_column_count(stmt) };
                        let row = (0..n)
                            .map(|i| unsafe {
                                match sqlite3_column_type(stmt, i) {
                                    1 => Val::Int(sqlite3_column_int64(stmt, i)),
                                    2 => Val::Real(sqlite3_column_double(stmt, i)),
                                    5 => Val::Null,
                                    _ => {
                                        let p = sqlite3_column_text(stmt, i);
                                        let len = sqlite3_column_bytes(stmt, i) as usize;
                                        let bytes = if p.is_null() { &[][..] } else { std::slice::from_raw_parts(p, len) };
                                        Val::Text(String::from_utf8_lossy(bytes).into_owned())
                                    }
                                }
                            })
                            .collect();
                        on_row(row);
                    }
                    DONE => return Ok(()),
                    _ => return Err(format!("{}: {sql}", self.err())),
                }
            }
        })();
        unsafe { sqlite3_finalize(stmt) };
        result
    }
}

impl Drop for Db {
    fn drop(&mut self) {
        unsafe { sqlite3_close_v2(self.raw) };
    }
}
