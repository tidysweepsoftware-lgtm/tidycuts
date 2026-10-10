//! A small HTTP server for the MCP "streamable HTTP" transport.
//!
//! It listens on the loopback address only, answers `POST /mcp` with the
//! JSON-RPC reply, and refuses anything that doesn't carry the secret token,
//! names another host (DNS rebinding) or comes from a web page (an `Origin`
//! that isn't this computer). Each connection gets its own thread and is
//! closed after one request, which every MCP client copes with.

use crate::protocol::{handle_body, Host, Reply};
use serde_json::{json, Value};
use std::io::{self, Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

const MAX_HEADER: usize = 16 * 1024;
const MAX_BODY: usize = 4 * 1024 * 1024;
const MAX_CONNECTIONS: usize = 16;
const READ_TIMEOUT: Duration = Duration::from_secs(30);

/// The path the server answers on.
pub const ENDPOINT: &str = "/mcp";

/// A running server. Dropping it stops it.
pub struct Server {
    port: u16,
    token: String,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Server {
    /// Starts listening on 127.0.0.1 (`port` 0 picks a free one). `token` is
    /// what clients must send as `Authorization: Bearer <token>`.
    pub fn start(host: Arc<dyn Host>, token: String, port: u16) -> io::Result<Server> {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, port))?;
        let port = listener.local_addr()?.port();
        let stop = Arc::new(AtomicBool::new(false));
        let busy = Arc::new(AtomicUsize::new(0));
        let (stop2, token2) = (stop.clone(), token.clone());
        let thread = thread::Builder::new()
            .name("tidycuts-mcp".into())
            .spawn(move || {
                for stream in listener.incoming() {
                    if stop2.load(Ordering::SeqCst) {
                        break;
                    }
                    let Ok(stream) = stream else { continue };
                    if busy.fetch_add(1, Ordering::SeqCst) >= MAX_CONNECTIONS {
                        busy.fetch_sub(1, Ordering::SeqCst);
                        let _ = respond(&stream, 503, "Service Unavailable", &[], b"");
                        continue;
                    }
                    let (host, token, busy) = (host.clone(), token2.clone(), busy.clone());
                    let spawned = thread::Builder::new()
                        .name("tidycuts-mcp-conn".into())
                        .spawn(move || {
                            if let Err(e) = serve(stream, &*host, &token, port) {
                                log::debug!("mcp connection ended: {e}");
                            }
                            busy.fetch_sub(1, Ordering::SeqCst);
                        });
                    if spawned.is_err() {
                        log::warn!("could not start an mcp connection thread");
                    }
                }
            })?;
        Ok(Server {
            port,
            token,
            stop,
            thread: Some(thread),
        })
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn token(&self) -> &str {
        &self.token
    }

    /// Where clients connect.
    pub fn url(&self) -> String {
        format!("http://127.0.0.1:{}{ENDPOINT}", self.port)
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        // Wake the accept loop so it notices.
        let _ = TcpStream::connect(SocketAddr::from((Ipv4Addr::LOCALHOST, self.port)));
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

/// A fresh secret: 256 random bits as hex.
pub fn new_token() -> String {
    format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    )
}

struct Request {
    method: String,
    path: String,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

impl Request {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
    }
}

fn serve(mut stream: TcpStream, host: &dyn Host, token: &str, port: u16) -> io::Result<()> {
    stream.set_read_timeout(Some(READ_TIMEOUT))?;
    stream.set_write_timeout(Some(READ_TIMEOUT))?;
    let req = match read_request(&mut stream) {
        Ok(r) => r,
        Err(Rejected(status, reason)) => {
            return respond(&stream, status, reason, &[], reason.as_bytes());
        }
    };
    if let Some((status, reason, extra)) = check(&req, token, port) {
        return respond(&stream, status, reason, &extra, reason.as_bytes());
    }
    let path = req.path.split('?').next().unwrap_or("");
    if path != ENDPOINT {
        return respond(&stream, 404, "Not Found", &[], b"Not found");
    }
    if req.method != "POST" {
        // No server-sent event stream and no sessions to end.
        return respond(
            &stream,
            405,
            "Method Not Allowed",
            &[("Allow", "POST")],
            b"Use POST",
        );
    }
    match handle_body(host, &req.body) {
        Reply::Accepted => respond(&stream, 202, "Accepted", &[], b""),
        Reply::Json(v) => {
            let body = serde_json::to_vec(&v).unwrap_or_default();
            respond(
                &stream,
                200,
                "OK",
                &[("Content-Type", "application/json")],
                &body,
            )
        }
    }
}

type Refusal = (u16, &'static str, Vec<(&'static str, &'static str)>);

/// The checks every request passes before anything is run: the host name,
/// where it came from and the token.
fn check(req: &Request, token: &str, port: u16) -> Option<Refusal> {
    let good_host = |h: &str| {
        let h = h.to_ascii_lowercase();
        [
            format!("127.0.0.1:{port}"),
            format!("localhost:{port}"),
            format!("[::1]:{port}"),
        ]
        .contains(&h)
    };
    match req.header("host") {
        Some(h) if good_host(h) => {}
        _ => return Some((403, "Forbidden", vec![])),
    }
    if let Some(origin) = req.header("origin") {
        let o = origin.to_ascii_lowercase();
        let local = ["http://127.0.0.1", "http://localhost", "http://[::1]"]
            .iter()
            .any(|p| o == *p || o.starts_with(&format!("{p}:")));
        if !local {
            return Some((403, "Forbidden", vec![]));
        }
    }
    let given = req
        .header("authorization")
        .and_then(|a| {
            a.strip_prefix("Bearer ")
                .or_else(|| a.strip_prefix("bearer "))
        })
        .unwrap_or("");
    if !same(given.as_bytes(), token.as_bytes()) {
        return Some((
            401,
            "Unauthorized",
            vec![("WWW-Authenticate", "Bearer realm=\"tidycuts\"")],
        ));
    }
    None
}

/// Compares without stopping at the first difference.
fn same(a: &[u8], b: &[u8]) -> bool {
    let mut diff = (a.len() != b.len()) as u8;
    for i in 0..a.len().max(b.len()) {
        diff |= a.get(i).copied().unwrap_or(0) ^ b.get(i).copied().unwrap_or(0);
    }
    diff == 0
}

struct Rejected(u16, &'static str);

fn read_request(stream: &mut TcpStream) -> Result<Request, Rejected> {
    let bad = |_: io::Error| Rejected(400, "Bad Request");
    let mut buf = Vec::with_capacity(2048);
    let mut chunk = [0u8; 2048];
    let end = loop {
        if let Some(i) = find(&buf, b"\r\n\r\n") {
            break i;
        }
        if buf.len() > MAX_HEADER {
            return Err(Rejected(431, "Request Header Fields Too Large"));
        }
        let n = stream.read(&mut chunk).map_err(bad)?;
        if n == 0 {
            return Err(Rejected(400, "Bad Request"));
        }
        buf.extend_from_slice(&chunk[..n]);
    };
    let head = std::str::from_utf8(&buf[..end]).map_err(|_| Rejected(400, "Bad Request"))?;
    let mut lines = head.split("\r\n");
    let mut first = lines.next().unwrap_or("").split(' ');
    let (method, path) = match (first.next(), first.next()) {
        (Some(m), Some(p)) if !m.is_empty() && p.starts_with('/') => (m.to_string(), p.to_string()),
        _ => return Err(Rejected(400, "Bad Request")),
    };
    let headers: Vec<(String, String)> = lines
        .filter_map(|l| l.split_once(':'))
        .map(|(k, v)| (k.trim().to_ascii_lowercase(), v.trim().to_string()))
        .collect();
    let find_header = |name: &str| headers.iter().find(|(k, _)| k == name).map(|(_, v)| v);
    if find_header("transfer-encoding").is_some() {
        return Err(Rejected(411, "Length Required"));
    }
    let len: usize = match find_header("content-length") {
        Some(v) => v.parse().map_err(|_| Rejected(400, "Bad Request"))?,
        None => 0,
    };
    if len > MAX_BODY {
        return Err(Rejected(413, "Payload Too Large"));
    }
    let mut body = buf[end + 4..].to_vec();
    while body.len() < len {
        let n = stream.read(&mut chunk).map_err(bad)?;
        if n == 0 {
            return Err(Rejected(400, "Bad Request"));
        }
        body.extend_from_slice(&chunk[..n]);
    }
    body.truncate(len);
    Ok(Request {
        method,
        path,
        headers,
        body,
    })
}

fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}

fn respond(
    mut stream: &TcpStream,
    status: u16,
    reason: &str,
    headers: &[(&str, &str)],
    body: &[u8],
) -> io::Result<()> {
    let mut head = format!("HTTP/1.1 {status} {reason}\r\n");
    for (k, v) in headers {
        head.push_str(&format!("{k}: {v}\r\n"));
    }
    head.push_str(&format!(
        "Content-Length: {}\r\nConnection: close\r\nCache-Control: no-store\r\n\r\n",
        body.len()
    ));
    stream.write_all(head.as_bytes())?;
    stream.write_all(body)?;
    stream.flush()
}

/// A JSON-RPC error body for tests and callers that want one.
pub fn rpc_error(code: i64, message: &str) -> Value {
    json!({"jsonrpc": "2.0", "id": null, "error": {"code": code, "message": message}})
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{Tool, ToolResult};

    struct Fake;

    impl Host for Fake {
        fn instructions(&self) -> String {
            "hi".into()
        }
        fn tools(&self) -> Vec<Tool> {
            vec![Tool {
                name: "echo".into(),
                description: "echo".into(),
                schema: json!({"type": "object"}),
                read_only: true,
            }]
        }
        fn call(&self, _: &str, args: &Value) -> ToolResult {
            ToolResult::json(args)
        }
    }

    fn server() -> Server {
        Server::start(Arc::new(Fake), "sekret".into(), 0).unwrap()
    }

    /// Sends raw text, returns (status, headers text, body).
    fn send(port: u16, raw: &str) -> (u16, String, String) {
        let mut s = TcpStream::connect((Ipv4Addr::LOCALHOST, port)).unwrap();
        s.write_all(raw.as_bytes()).unwrap();
        let mut out = String::new();
        s.read_to_string(&mut out).unwrap();
        let (head, body) = out.split_once("\r\n\r\n").unwrap_or((&out, ""));
        let status = head.split(' ').nth(1).unwrap().parse().unwrap();
        (status, head.to_string(), body.to_string())
    }

    fn post(port: u16, extra: &str, body: &str) -> (u16, String, String) {
        send(
            port,
            &format!(
                "POST /mcp HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n{extra}Content-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
                body.len()
            ),
        )
    }

    const AUTH: &str = "Authorization: Bearer sekret\r\n";

    #[test]
    fn a_client_with_the_token_can_list_and_call_tools() {
        let s = server();
        let (status, head, body) = post(
            s.port(),
            AUTH,
            r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#,
        );
        assert_eq!(status, 200);
        assert!(head.to_ascii_lowercase().contains("application/json"));
        let v: Value = serde_json::from_str(&body).unwrap();
        assert_eq!(v["result"]["tools"][0]["name"], "echo");
        let (_, _, body) = post(
            s.port(),
            AUTH,
            r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"echo","arguments":{"x":5}}}"#,
        );
        let v: Value = serde_json::from_str(&body).unwrap();
        assert!(v["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("\"x\": 5"));
        assert!(s.url().starts_with("http://127.0.0.1:"));
    }

    #[test]
    fn notifications_are_accepted_without_a_body() {
        let s = server();
        let (status, _, body) = post(
            s.port(),
            AUTH,
            r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
        );
        assert_eq!((status, body.as_str()), (202, ""));
    }

    #[test]
    fn requests_without_the_right_token_are_refused() {
        let s = server();
        let msg = r#"{"jsonrpc":"2.0","id":1,"method":"ping"}"#;
        let (status, head, _) = post(s.port(), "", msg);
        assert_eq!(status, 401);
        assert!(head.contains("WWW-Authenticate"));
        let (status, _, _) = post(s.port(), "Authorization: Bearer nope\r\n", msg);
        assert_eq!(status, 401);
        let (status, _, _) = post(s.port(), "Authorization: Bearer sekre\r\n", msg);
        assert_eq!(status, 401);
    }

    #[test]
    fn web_pages_and_other_host_names_are_refused() {
        let s = server();
        let msg = r#"{"jsonrpc":"2.0","id":1,"method":"ping"}"#;
        let (status, _, _) = post(
            s.port(),
            &format!("{AUTH}Origin: https://evil.example\r\n"),
            msg,
        );
        assert_eq!(status, 403);
        // A page on this computer is no worse than any other local program.
        let (status, _, _) = post(
            s.port(),
            &format!("{AUTH}Origin: http://localhost:3000\r\n"),
            msg,
        );
        assert_eq!(status, 200);
        // DNS rebinding: a name that points at 127.0.0.1.
        let raw = format!(
            "POST /mcp HTTP/1.1\r\nHost: evil.example:{}\r\n{AUTH}Content-Length: {}\r\n\r\n{msg}",
            s.port(),
            msg.len()
        );
        assert_eq!(send(s.port(), &raw).0, 403);
    }

    #[test]
    fn other_methods_paths_and_sizes_are_handled() {
        let s = server();
        let p = s.port();
        let get = format!("GET /mcp HTTP/1.1\r\nHost: 127.0.0.1:{p}\r\n{AUTH}\r\n");
        let (status, head, _) = send(p, &get);
        assert_eq!(status, 405);
        assert!(head.contains("Allow: POST"));
        let wrong = format!(
            "POST /other HTTP/1.1\r\nHost: 127.0.0.1:{p}\r\n{AUTH}Content-Length: 0\r\n\r\n"
        );
        assert_eq!(send(p, &wrong).0, 404);
        let huge = format!(
            "POST /mcp HTTP/1.1\r\nHost: 127.0.0.1:{p}\r\n{AUTH}Content-Length: 99999999\r\n\r\n"
        );
        assert_eq!(send(p, &huge).0, 413);
        let chunked = format!(
            "POST /mcp HTTP/1.1\r\nHost: 127.0.0.1:{p}\r\n{AUTH}Transfer-Encoding: chunked\r\n\r\n"
        );
        assert_eq!(send(p, &chunked).0, 411);
        assert_eq!(send(p, "nonsense\r\n\r\n").0, 400);
    }

    #[test]
    fn tokens_are_long_and_different_each_time() {
        let (a, b) = (new_token(), new_token());
        assert_eq!(a.len(), 64);
        assert_ne!(a, b);
        assert!(same(b"abc", b"abc") && !same(b"abc", b"abd") && !same(b"abc", b"ab"));
        assert_eq!(rpc_error(-1, "x")["error"]["code"], -1);
    }

    #[test]
    fn dropping_the_server_stops_it() {
        let s = server();
        let p = s.port();
        drop(s);
        assert!(TcpStream::connect((Ipv4Addr::LOCALHOST, p)).is_err());
    }
}
