//! Local registry proxy used for `fdn://` navigation.
//!
//! CEF does not ship a custom-scheme resource handler in this prototype, so
//! registry pages are served through a loopback HTTP proxy using a wildcard
//! `*.fdn.localhost` host. That preserves the page's origin per registry name,
//! keeps relative links working, and lets the proxy rewrite absolute links back
//! into the registry namespace.

use std::io;
use std::net::TcpListener as StdTcpListener;
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

const PROXY_SUFFIX: &str = ".localhost";
const MAX_HEADER_BYTES: usize = 64 * 1024;
const MAX_BODY_BYTES: usize = 64 * 1024 * 1024;

static PORT: OnceLock<u16> = OnceLock::new();
static STATE: OnceLock<Arc<ProxyState>> = OnceLock::new();

struct ProxyState {
    port: u16,
    http: reqwest::Client,
    registry: fdn_core::RegistryClient,
}

/// Start the loopback proxy once. Safe to call from the browser process only.
pub fn start() {
    if PORT.get().is_some() {
        return;
    }
    let listener = match StdTcpListener::bind(("127.0.0.1", 0)) {
        Ok(listener) => listener,
        Err(error) => {
            eprintln!("[proxy] could not bind loopback proxy: {error}");
            return;
        }
    };
    let port = match listener.local_addr() {
        Ok(address) => address.port(),
        Err(error) => {
            eprintln!("[proxy] could not read proxy address: {error}");
            return;
        }
    };
    if listener.set_nonblocking(true).is_err() {
        eprintln!("[proxy] could not make listener nonblocking");
        return;
    }
    let cache = std::env::temp_dir().join("fdn-registry");
    let registry = match fdn_core::RegistryClient::new(cache) {
        Ok(registry) => registry,
        Err(error) => {
            eprintln!("[proxy] registry cache unavailable: {error}");
            return;
        }
    };
    let http = match reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(45))
        .user_agent(concat!(
            "FreeDomainBrowser/",
            env!("CARGO_PKG_VERSION"),
            " (+https://github.com/free-domain-name/registry)"
        ))
        .build()
    {
        Ok(http) => http,
        Err(error) => {
            eprintln!("[proxy] HTTP client unavailable: {error}");
            return;
        }
    };
    let state = Arc::new(ProxyState {
        port,
        http,
        registry,
    });
    if STATE.set(state.clone()).is_err() {
        return;
    }
    if PORT.set(port).is_err() {
        return;
    }
    let _ = std::thread::Builder::new()
        .name("fdn-proxy".to_string())
        .spawn(move || {
            let runtime = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(runtime) => runtime,
                Err(error) => {
                    eprintln!("[proxy] runtime unavailable: {error}");
                    return;
                }
            };
            runtime.block_on(async move {
                let listener = match TcpListener::from_std(listener) {
                    Ok(listener) => listener,
                    Err(error) => {
                        eprintln!("[proxy] listener unavailable: {error}");
                        return;
                    }
                };
                loop {
                    match listener.accept().await {
                        Ok((stream, _)) => {
                            let state = state.clone();
                            tokio::spawn(async move {
                                let _ = handle(stream, state).await;
                            });
                        }
                        Err(error) => {
                            eprintln!("[proxy] accept failed: {error}");
                            tokio::time::sleep(Duration::from_millis(100)).await;
                        }
                    }
                }
            });
        });
}

/// URL for a registry name, or `None` when the proxy did not start.
pub fn proxy_url(domain: &str, path: &str) -> Option<String> {
    let port = PORT.get().copied()?;
    proxy_url_for_port(domain, path, port)
}

fn proxy_url_for_port(domain: &str, path: &str, port: u16) -> Option<String> {
    let domain = domain.trim().trim_end_matches('.').to_ascii_lowercase();
    if domain.is_empty() {
        return None;
    }
    let path = normalize_path(path);
    Some(format!("http://{domain}{PROXY_SUFFIX}:{port}{path}"))
}

/// Turn a loopback proxy URL back into the address shown in the chrome.
pub fn display_url(raw: &str) -> Option<String> {
    let parsed = url::Url::parse(raw).ok()?;
    if parsed.scheme() != "http" {
        return None;
    }
    let host = parsed.host_str()?;
    let domain = host.strip_suffix(PROXY_SUFFIX)?;
    if domain.is_empty() {
        return None;
    }
    let mut path = parsed.path().to_string();
    if let Some(query) = parsed.query() {
        path.push('?');
        path.push_str(query);
    }
    if let Some(fragment) = parsed.fragment() {
        path.push('#');
        path.push_str(fragment);
    }
    Some(format!("fdn://{domain}{path}"))
}

/// Clear both the disk lookup cache and the long-lived in-memory cache.
pub fn clear_cache() {
    let _ = std::fs::remove_dir_all(std::env::temp_dir().join("fdn-registry"));
    let Some(state) = STATE.get().cloned() else {
        return;
    };
    let _ = std::thread::Builder::new()
        .name("fdn-proxy-clear".to_string())
        .spawn(move || {
            let runtime = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(runtime) => runtime,
                Err(_) => return,
            };
            runtime.block_on(async move {
                let _ = state.registry.clear().await;
            });
        });
}

fn normalize_path(raw: &str) -> String {
    let mut path = raw.trim().to_string();
    if path.is_empty() {
        return "/".to_string();
    }
    if !path.starts_with('/') {
        path.insert(0, '/');
    }
    path.retain(|character| !character.is_control());
    path
}

fn proxy_domain(host: &str) -> Option<&str> {
    let host = host.rsplit_once(':').map(|(host, _)| host).unwrap_or(host);
    host.strip_suffix(PROXY_SUFFIX)
        .filter(|domain| !domain.is_empty())
}

struct ParsedRequest {
    method: String,
    target: String,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

impl ParsedRequest {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }
}

async fn handle(mut stream: TcpStream, state: Arc<ProxyState>) -> io::Result<()> {
    let request = match read_request(&mut stream).await {
        Ok(Some(request)) => request,
        Ok(None) => return Ok(()),
        Err(error) => {
            let _ = write_response(
                &mut stream,
                simple_error(400, "Bad Request", &html_escape(&error)),
            )
            .await;
            return Ok(());
        }
    };
    let Some(host) = request.header("host") else {
        let _ = write_response(
            &mut stream,
            simple_error(400, "Bad Request", "missing Host header"),
        )
        .await;
        return Ok(());
    };
    let Some(domain) = proxy_domain(host) else {
        let _ = write_response(
            &mut stream,
            simple_error(421, "Misdirected Request", "unknown proxy host"),
        )
        .await;
        return Ok(());
    };
    let response = match fetch(&state, domain, &request).await {
        Ok(response) => response,
        Err(error) => simple_error(502, "Bad Gateway", &html_escape(&error)),
    };
    write_response(&mut stream, response).await
}

async fn read_request(stream: &mut TcpStream) -> Result<Option<ParsedRequest>, String> {
    let mut buffer = Vec::new();
    let mut chunk = [0u8; 8192];
    let header_end = loop {
        if let Some(position) = find_bytes(&buffer, b"\r\n\r\n") {
            break position;
        }
        if buffer.len() > MAX_HEADER_BYTES {
            return Err("request headers are too large".to_string());
        }
        let read = stream.read(&mut chunk).await.map_err(|error| error.to_string())?;
        if read == 0 {
            if buffer.is_empty() {
                return Ok(None);
            }
            return Err("incomplete request".to_string());
        }
        buffer.extend_from_slice(&chunk[..read]);
    };
    let head = String::from_utf8_lossy(&buffer[..header_end]).to_string();
    let mut lines = head.split("\r\n");
    let request_line = lines.next().ok_or("missing request line")?;
    let mut parts = request_line.split_whitespace();
    let method = parts.next().ok_or("missing method")?.to_string();
    let target = parts.next().ok_or("missing target")?.to_string();
    let headers = lines
        .filter_map(|line| line.split_once(':'))
        .map(|(name, value)| (name.trim().to_string(), value.trim().to_string()))
        .collect::<Vec<_>>();
    let content_length = headers
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("content-length"))
        .and_then(|(_, value)| value.parse::<usize>().ok())
        .unwrap_or(0);
    if content_length > MAX_BODY_BYTES {
        return Err("request body is too large".to_string());
    }
    let mut body = buffer[header_end + 4..].to_vec();
    while body.len() < content_length {
        let read = stream.read(&mut chunk).await.map_err(|error| error.to_string())?;
        if read == 0 {
            return Err("incomplete request body".to_string());
        }
        body.extend_from_slice(&chunk[..read]);
    }
    body.truncate(content_length);
    Ok(Some(ParsedRequest {
        method,
        target,
        headers,
        body,
    }))
}

async fn fetch(
    state: &ProxyState,
    domain: &str,
    request: &ParsedRequest,
) -> Result<Vec<u8>, String> {
    let record = state
        .registry
        .lookup(domain)
        .await
        .map_err(|error| format!("registry lookup failed: {error}"))?
        .ok_or_else(|| format!("{domain} is not registered"))?;
    let target = record.target_url.trim_end_matches('/').to_string();
    let upstream = if request.target.starts_with('/') {
        format!("{target}{}", request.target)
    } else {
        target.clone()
    };
    let parsed_target = url::Url::parse(&target).map_err(|error| error.to_string())?;
    let upstream_host = if parsed_target.host_str().is_some_and(|host| host.parse::<std::net::IpAddr>().is_ok()) {
        domain.to_string()
    } else {
        parsed_target
            .host_str()
            .unwrap_or_default()
            .to_string()
    };

    let method = reqwest::Method::from_bytes(request.method.as_bytes())
        .map_err(|_| "unsupported method".to_string())?;
    let mut builder = state.http.request(method, &upstream);
    builder = builder.header("Host", upstream_host);
    for (name, value) in &request.headers {
        if skip_request_header(name) {
            continue;
        }
        // Every request reaching this proxy is same-origin under
        // `{domain}.fdn.localhost`, so `Cookie` always rides along. Third-party
        // blocking is enforced once, on the CEF handler path
        // (`on_before_resource_load`), keeping both paths consistent with the
        // "Block third-party cookies" checkbox. (Previously the checkbox was
        // read inverted here and dropped cookies exactly when blocking was off.)
        if name.eq_ignore_ascii_case("origin") || name.eq_ignore_ascii_case("referer") {
            let rewritten = rewrite_referrer(value, &target, domain, state.port);
            builder = builder.header(name, rewritten);
            continue;
        }
        builder = builder.header(name, value);
    }
    if crate::settings::load().dnt {
        builder = builder.header("DNT", "1");
    }
    if !request.body.is_empty() {
        builder = builder.body(request.body.clone());
    }
    let response = builder
        .send()
        .await
        .map_err(|error| format!("upstream request failed: {error}"))?;
    let status = response.status();
    let headers = response.headers().clone();
    let body = response
        .bytes()
        .await
        .map_err(|error| format!("upstream body failed: {error}"))?;
    if body.len() > MAX_BODY_BYTES {
        return Err("upstream response is too large".to_string());
    }
    let content_type = headers
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("");
    let body = if content_type.to_ascii_lowercase().contains("text/html") {
        rewrite_html(&body, &target, domain, state.port).into_bytes()
    } else {
        body.to_vec()
    };

    let mut response = format!(
        "HTTP/1.1 {} {}\r\n",
        status.as_u16(),
        status.canonical_reason().unwrap_or("OK")
    );
    for (name, value) in headers.iter() {
        let name_string = name.as_str();
        if skip_response_header(name_string) {
            continue;
        }
        let Ok(value) = value.to_str() else {
            continue;
        };
        let value = if name_string.eq_ignore_ascii_case("location") {
            rewrite_location(value, &target, domain, state.port)
        } else if name_string.eq_ignore_ascii_case("set-cookie") {
            sanitize_cookie(value)
        } else {
            value.to_string()
        };
        response.push_str(name_string);
        response.push_str(": ");
        response.push_str(&value);
        response.push_str("\r\n");
    }
    response.push_str(&format!(
        "Content-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    ));
    let mut bytes = response.into_bytes();
    bytes.extend_from_slice(&body);
    Ok(bytes)
}

fn skip_request_header(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "host"
            | "connection"
            | "proxy-connection"
            | "transfer-encoding"
            | "upgrade"
            | "content-length"
            | "accept-encoding"
    )
}

fn skip_response_header(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "connection"
            | "content-length"
            | "content-encoding"
            | "transfer-encoding"
            | "content-security-policy"
            | "content-security-policy-report-only"
            | "x-frame-options"
            | "cross-origin-opener-policy"
            | "cross-origin-embedder-policy"
            | "origin-agent-cluster"
            | "strict-transport-security"
            | "alt-svc"
            | "report-to"
            | "nel"
    )
}

fn sanitize_cookie(value: &str) -> String {
    value
        .split(';')
        .map(str::trim)
        .filter(|part| {
            let lower = part.to_ascii_lowercase();
            !lower.starts_with("domain=") && lower != "secure"
        })
        .collect::<Vec<_>>()
        .join("; ")
}

fn rewrite_html(body: &[u8], target: &str, domain: &str, port: u16) -> String {
    let proxy = format!("http://{domain}{PROXY_SUFFIX}:{port}");
    let mut text = String::from_utf8_lossy(body).into_owned();
    if let Ok(parsed) = url::Url::parse(target) {
        if let Some(host) = parsed.host_str() {
            text = text.replace(
                &format!("{}://{host}", parsed.scheme()),
                &proxy,
            );
            text = text.replace(
                &format!("{}://{host}", if parsed.scheme() == "https" { "http" } else { "https" }),
                &proxy,
            );
            text = text.replace(&format!("//{host}"), &format!("//{domain}{PROXY_SUFFIX}:{port}"));
        }
    }
    text = text.replace(target.trim_end_matches('/'), &proxy);
    text
}

fn rewrite_referrer(value: &str, target: &str, domain: &str, port: u16) -> String {
    let proxy = format!("http://{domain}{PROXY_SUFFIX}:{port}");
    let target = target.trim_end_matches('/');
    value.replace(&proxy, target)
}

fn rewrite_location(value: &str, target: &str, domain: &str, port: u16) -> String {
    let proxy = format!("http://{domain}{PROXY_SUFFIX}:{port}");
    let target = target.trim_end_matches('/');
    if let Some(rest) = value.strip_prefix(target) {
        return format!("{proxy}{rest}");
    }
    if let Ok(parsed) = url::Url::parse(value) {
        if parsed.host_str().is_some_and(|host| target.contains(host)) {
            let mut rewritten = format!("{proxy}{}", parsed.path());
            if let Some(query) = parsed.query() {
                rewritten.push('?');
                rewritten.push_str(query);
            }
            return rewritten;
        }
    }
    value.to_string()
}

async fn write_response(stream: &mut TcpStream, response: Vec<u8>) -> io::Result<()> {
    stream.write_all(&response).await
}

fn simple_error(status: u16, title: &str, detail: &str) -> Vec<u8> {
    let body = format!(
        "<!doctype html><html><body style=\"font:15px system-ui;background:#10151f;color:#e8edf7;padding:48px\"><h1>{}</h1><p>{}</p></body></html>",
        html_escape(title),
        detail
    );
    format!(
        "HTTP/1.1 {status} {}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        html_escape(title),
        body.len(),
        body
    )
    .into_bytes()
}

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

#[cfg(test)]
mod tests {
    use super::{
        display_url, normalize_path, proxy_domain, proxy_url_for_port, sanitize_cookie,
        skip_request_header, skip_response_header,
    };

    #[test]
    fn proxy_url_round_trips_to_a_display_url() {
        let url = proxy_url_for_port("hello.fdn", "/path?q=1", 4567).unwrap();
        assert_eq!(display_url(&url).as_deref(), Some("fdn://hello.fdn/path?q=1"));
    }

    #[test]
    fn recognizes_only_registry_proxy_hosts() {
        assert_eq!(proxy_domain("hello.fdn.localhost:1234"), Some("hello.fdn"));
        assert_eq!(proxy_domain("example.com"), None);
    }

    #[test]
    fn normalizes_paths_and_cookies() {
        assert_eq!(normalize_path(""), "/");
        assert_eq!(normalize_path("a"), "/a");
        assert_eq!(sanitize_cookie("a=1; Domain=x; Secure; Path=/"), "a=1; Path=/");
    }

    // Every proxy request is same-origin, so `Cookie` must survive the proxy
    // path; third-party blocking lives only in the CEF request handler.
    #[test]
    fn cookies_are_never_stripped_by_the_proxy() {
        assert!(!skip_request_header("cookie"));
        assert!(!skip_response_header("set-cookie"));
        // Sanitization still drops the domain/secure attributes.
        assert_eq!(sanitize_cookie("a=1; domain=.example.com; Secure"), "a=1");
    }
}
