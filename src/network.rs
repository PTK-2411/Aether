use std::net::Ipv4Addr;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread;
use url::Url;

#[derive(Debug, Clone)]
pub struct PageResponse {
    pub url: String,
    pub html: String,
    pub external_css: Vec<String>,
    pub title: String,
    pub status: u16,
    pub status_text: String,
    pub headers: Vec<(String, String)>,
    pub duration_ms: u64,
}

pub enum NetworkMessage {
    PageLoaded {
        tab_id: usize,
        result: Result<PageResponse, LoadError>,
    },
}

pub struct NetworkManager {
    sender: Sender<NetworkMessage>,
    receiver: Receiver<NetworkMessage>,
}

impl NetworkManager {
    pub fn new() -> Self {
        let (sender, receiver) = channel();
        NetworkManager { sender, receiver }
    }

    pub fn try_recv(&self) -> Option<NetworkMessage> {
        self.receiver.try_recv().ok()
    }

    pub fn fetch(&self, tab_id: usize, raw_url: &str) {
        let tx = self.sender.clone();
        let target_url = normalize_url(raw_url);

        thread::spawn(move || {
            let res = fetch_url_content(&target_url);
            let _ = tx.send(NetworkMessage::PageLoaded {
                tab_id,
                result: res,
            });
        });
    }
}

fn is_valid_domain_or_ip(trimmed: &str) -> bool {
    if trimmed.chars().any(|c| c.is_whitespace()) {
        return false;
    }

    let authority = match trimmed.find(|c| c == '/' || c == '?' || c == '#') {
        Some(idx) => &trimmed[..idx],
        None => trimmed,
    };

    if authority.is_empty() {
        return false;
    }

    let host = match authority.rsplit_once(':') {
        Some((h, port_str)) => {
            if port_str.parse::<u16>().is_ok() {
                h
            } else {
                return false;
            }
        }
        None => authority,
    };

    if host.parse::<Ipv4Addr>().is_ok() {
        return true;
    }

    if !host.contains('.') || host.starts_with('.') || host.ends_with('.') {
        return false;
    }

    let labels: Vec<&str> = host.split('.').collect();
    if labels.len() < 2 {
        return false;
    }

    for label in &labels {
        if label.is_empty() {
            return false;
        }
        if !label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
            return false;
        }
        if label.starts_with('-') || label.ends_with('-') {
            return false;
        }
    }

    let tld = labels.last().unwrap();
    if tld.len() < 2 || !tld.chars().all(|c| c.is_ascii_alphabetic()) {
        return false;
    }

    true
}

pub fn normalize_url(input: &str) -> String {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return "about:newtab".to_string();
    }

    if trimmed == "about:welcome" || trimmed == "about:newtab" || trimmed == "about:home" {
        return "about:newtab".to_string();
    }

    if trimmed.starts_with("about:") {
        return trimmed.to_string();
    }

    if trimmed.starts_with("file://") {
        return trimmed.to_string();
    }

    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        return trimmed.to_string();
    }

    // Localhost handling (e.g. "localhost", "localhost:8080", "localhost/dashboard")
    if trimmed == "localhost" || trimmed.starts_with("localhost:") || trimmed.starts_with("localhost/") {
        return format!("http://{}", trimmed);
    }

    // If it looks like a valid domain or IPv4 address, navigate directly via HTTPS
    if is_valid_domain_or_ip(trimmed) {
        return format!("https://{}", trimmed);
    }

    // Otherwise treat as search query on Google Search
    let encoded = url::form_urlencoded::byte_serialize(trimmed.as_bytes()).collect::<String>();
    format!("https://www.google.com/search?q={}", encoded)
}

pub fn resolve_relative_url(base_url: &str, href: &str) -> String {
    let trimmed_href = href.trim();
    if trimmed_href.starts_with("about:")
        || trimmed_href.starts_with("http://")
        || trimmed_href.starts_with("https://")
        || trimmed_href.starts_with("file://")
    {
        return trimmed_href.to_string();
    }

    if trimmed_href.starts_with("//") {
        if base_url.starts_with("http://") {
            return format!("http:{}", trimmed_href);
        } else {
            return format!("https:{}", trimmed_href);
        }
    }

    if let Ok(base) = Url::parse(base_url) {
        if let Ok(resolved) = base.join(trimmed_href) {
            return resolved.to_string();
        }
    }

    trimmed_href.to_string()
}

#[derive(Debug, Clone)]
pub struct LoadError {
    /// Chrome-style net error code, e.g. `ERR_NAME_NOT_RESOLVED`.
    pub code: String,
    /// Heading shown on the error page, e.g. "This site can't be reached".
    pub title: String,
    /// One-sentence explanation of what went wrong.
    pub description: String,
    /// Underlying transport message, kept for DevTools/console.
    pub detail: String,
    pub url: String,
}

impl std::fmt::Display for LoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.description)
    }
}

impl LoadError {
    /// Practical next step, shown under the error code like Chrome does.
    pub fn hint(&self) -> &'static str {
        match self.code.as_str() {
            "ERR_NAME_NOT_RESOLVED" => {
                "Try checking the address for typos, or verify that your network's DNS server is reachable."
            }
            "ERR_CONNECTION_TIMED_OUT" => {
                "Try reloading once the server is back; a firewall or an idle host can cause this."
            }
            _ if self.code.starts_with("NET::ERR_CERT") => {
                "If you own this site, its certificate must list this hostname — a certificate issued for another domain will always be rejected."
            }
            "ERR_CONNECTION_REFUSED" => {
                "Make sure the server is running and accepting connections on this port."
            }
            "ERR_TOO_MANY_REDIRECTS" => {
                "Clearing cookies for this site usually breaks a redirect loop."
            }
            "ERR_FILE_NOT_FOUND" => {
                "Check the path, or drag the file onto the window to open it."
            }
            _ => "Try reloading the page, then check your internet connection.",
        }
    }
}

fn host_of(url_str: &str) -> String {
    Url::parse(url_str)
        .ok()
        .and_then(|u| u.host_str().map(|h| h.to_string()))
        .unwrap_or_else(|| url_str.to_string())
}

/// Turns a reqwest failure into the closest Chrome net-error code plus a
/// human-readable explanation, instead of leaking the raw transport string.
fn classify_load_error(err: &reqwest::Error, url_str: &str) -> LoadError {
    let mut detail = err.to_string();
    let mut source = std::error::Error::source(err);
    while let Some(s) = source {
        let msg = s.to_string();
        if !detail.contains(&msg) {
            detail.push_str(&format!(" — {}", msg));
        }
        source = std::error::Error::source(s);
    }
    let lower = detail.to_ascii_lowercase();
    let host = host_of(url_str);

    let (code, title, description) = if lower.contains("invalid peer certificate")
        || lower.contains("certificate")
        || lower.contains("rustls")
    {
        let (cert_code, reason) = if lower.contains("not valid for name")
            || lower.contains("common name")
            || lower.contains("hostname mismatch")
        {
            (
                "NET::ERR_CERT_COMMON_NAME_INVALID",
                format!(
                    "{}'s security certificate isn't valid for this hostname — it was issued for a different site, so the connection was blocked.",
                    host
                ),
            )
        } else if lower.contains("expired") {
            (
                "NET::ERR_CERT_DATE_INVALID",
                format!("{}'s security certificate has expired.", host),
            )
        } else {
            (
                "NET::ERR_CERT_AUTHORITY_INVALID",
                format!(
                    "{}'s security certificate isn't trusted, so the connection was blocked.",
                    host
                ),
            )
        };
        (cert_code, "Your connection isn't private", reason)
    } else if lower.contains("failed to lookup address information")
        || lower.contains("name or service not known")
        || lower.contains("nodename nor servname")
        || lower.contains("no address associated")
        || lower.contains("dns error")
    {
        (
            "ERR_NAME_NOT_RESOLVED",
            "This site can't be reached",
            format!("{}'s server IP address could not be found. Check the spelling, the network connection, or the DNS settings.", host),
        )
    } else if err.is_timeout() {
        (
            "ERR_CONNECTION_TIMED_OUT",
            "This site can't be reached",
            format!("{} took too long to respond.", host),
        )
    } else if lower.contains("connection refused") {
        (
            "ERR_CONNECTION_REFUSED",
            "This site can't be reached",
            format!("{} refused the connection.", host),
        )
    } else if lower.contains("reset") || lower.contains("broken pipe") {
        (
            "ERR_CONNECTION_RESET",
            "The connection was reset",
            format!("The connection to {} was closed unexpectedly.", host),
        )
    } else if lower.contains("invalid data") || lower.contains("malformed") {
        (
            "ERR_INVALID_RESPONSE",
            "This page isn't working",
            format!("{} sent a response that Aether couldn't understand.", host),
        )
    } else if err.is_redirect() {
        (
            "ERR_TOO_MANY_REDIRECTS",
            "The page isn't redirecting properly",
            format!("{} has redirected too many times to load the page.", host),
        )
    } else if err.is_decode() {
        (
            "ERR_CONTENT_DECODING_FAILED",
            "This page isn't working",
            format!("{} sent compressed data that couldn't be decoded.", host),
        )
    } else {
        (
            "ERR_NETWORK",
            "This site can't be reached",
            format!("Aether could not open the page at {} because the network connection failed.", host),
        )
    };

    LoadError {
        code: code.to_string(),
        title: title.to_string(),
        description,
        detail,
        url: url_str.to_string(),
    }
}

fn fetch_url_content(url_str: &str) -> Result<PageResponse, LoadError> {
    if url_str == "about:welcome" || url_str == "about:home" || url_str == "about:newtab" {
        return Ok(PageResponse {
            url: "about:newtab".to_string(),
            html: WELCOME_HTML.to_string(),
            external_css: Vec::new(),
            title: "New Tab".to_string(),
            status: 200,
            status_text: "OK".to_string(),
            headers: vec![("content-type".to_string(), "text/html; charset=utf-8".to_string())],
            duration_ms: 1,
        });
    }

    if url_str == "about:ai" || url_str.contains("gemini.google.com") {
        return Ok(PageResponse {
            url: "about:ai".to_string(),
            html: AI_MODE_HTML.to_string(),
            external_css: Vec::new(),
            title: "Aether AI Mode".to_string(),
            status: 200,
            status_text: "OK".to_string(),
            headers: vec![("content-type".to_string(), "text/html; charset=utf-8".to_string())],
            duration_ms: 1,
        });
    }

    if url_str == "about:test" || url_str == "about:acid" {
        return Ok(PageResponse {
            url: url_str.to_string(),
            html: TEST_SUITE_HTML.to_string(),
            external_css: Vec::new(),
            title: "CSS & DOM Test Suite".to_string(),
            status: 200,
            status_text: "OK".to_string(),
            headers: vec![("content-type".to_string(), "text/html; charset=utf-8".to_string())],
            duration_ms: 1,
        });
    }

    if url_str == "about:credits" {
        return Ok(PageResponse {
            url: url_str.to_string(),
            html: CREDITS_HTML.to_string(),
            external_css: Vec::new(),
            title: "Credits & Engine Architecture".to_string(),
            status: 200,
            status_text: "OK".to_string(),
            headers: vec![("content-type".to_string(), "text/html; charset=utf-8".to_string())],
            duration_ms: 1,
        });
    }

    if url_str == "about:blank" {
        return Ok(PageResponse {
            url: url_str.to_string(),
            html: "<html><body></body></html>".to_string(),
            external_css: Vec::new(),
            title: "Blank Tab".to_string(),
            status: 200,
            status_text: "OK".to_string(),
            headers: vec![("content-type".to_string(), "text/html; charset=utf-8".to_string())],
            duration_ms: 1,
        });
    }

    if let Some(file_path) = url_str.strip_prefix("file://") {
        match std::fs::read(file_path) {
            Ok(bytes) => {
                let content = decode_body(&bytes, None);
                let sheet_urls = extract_stylesheet_urls(&content);
                let mut external_css = Vec::new();
                let base_path = std::path::Path::new(file_path).parent().unwrap_or(std::path::Path::new(""));
                for raw_href in sheet_urls {
                    let resolved_path = base_path.join(&raw_href);
                    if let Ok(css_text) = std::fs::read_to_string(resolved_path) {
                        external_css.push(css_text);
                    }
                }

                return Ok(PageResponse {
                    url: url_str.to_string(),
                    title: file_path.to_string(),
                    html: content,
                    external_css,
                    status: 200,
                    status_text: "OK".to_string(),
                    headers: vec![("content-type".to_string(), "text/html; charset=utf-8".to_string())],
                    duration_ms: 1,
                });
            }
            Err(e) => {
                return Err(LoadError {
                    code: "ERR_FILE_NOT_FOUND".to_string(),
                    title: "This site isn't available".to_string(),
                    description: format!("Aether could not open the local file {}.", file_path),
                    detail: e.to_string(),
                    url: url_str.to_string(),
                });
            }
        }
    }

    // HTTP / HTTPS fetching via reqwest
    let start_time = std::time::Instant::now();
    let client = reqwest::blocking::Client::builder()
        .user_agent("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36 Aether/1.0")
        .timeout(std::time::Duration::from_secs(12))
        .build()
        .map_err(|e| LoadError {
            code: "ERR_INTERNET_DISCONNECTED".to_string(),
            title: "Aether can't connect to the network".to_string(),
            description: "The browser could not set up a secure connection.".to_string(),
            detail: e.to_string(),
            url: url_str.to_string(),
        })?;

    let response = client
        .get(url_str)
        .header("Accept", "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8")
        .header("Accept-Language", "en-US,en;q=0.9,vi;q=0.8")
        .send()
        .map_err(|e| classify_load_error(&e, url_str))?;
    let status = response.status().as_u16();
    let status_text = response.status().canonical_reason().unwrap_or("").to_string();
    let headers: Vec<(String, String)> = response
        .headers()
        .iter()
        .map(|(k, v)| (k.as_str().to_string(), v.to_str().unwrap_or("").to_string()))
        .collect();
    let final_url = response.url().to_string();
    let body_bytes = response.bytes().map_err(|e| classify_load_error(&e, url_str))?;
    let content_type = headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("content-type"))
        .map(|(_, v)| v.as_str());
    let body = decode_body(&body_bytes, content_type);
    let duration_ms = start_time.elapsed().as_millis() as u64;

    // Search engines answer a scriptless client with an empty shell. Rather
    // than painting a blank page, replay the same query against engines that
    // do serve plain HTML, and say on the page which one answered. Every
    // result below comes from a live engine; nothing is composed here.
    let (final_url, body) = if is_search_results_url(&final_url) {
        live_search_body(&final_url, body)
    } else {
        (final_url, body)
    };

    let page_title = extract_title_or(&body, &final_url);

    // Pages whose static HTML carries no text at all (JS renders everything)
    // get a plain explanation instead of a wall of empty skeleton placeholders.
    let final_html = {
        let visible = visible_text_len(&body);
        let lower = body.to_ascii_lowercase();
        let has_scripts = lower.contains("<script");
        let is_script_heavy = body.len() > 60_000
            || lower.contains("id=\"root\"")
            || lower.contains("id=\"app\"")
            || lower.contains("data-reactroot")
            || lower.contains("ng-app");
        if visible < 250 && has_scripts && is_script_heavy {
            embedded_json_content_html(&final_url, &body)
                .unwrap_or_else(|| spa_notice_html(&final_url, &page_title))
        } else {
            body.clone()
        }
    };
    let title = extract_title_or(&final_html, &final_url);

    let mut external_css = Vec::new();
    let sheet_urls = extract_stylesheet_urls(&final_html);
    for raw_href in sheet_urls {
        let resolved = resolve_relative_url(&final_url, &raw_href);
        if resolved.starts_with("http://") || resolved.starts_with("https://") {
            if let Ok(css_resp) = client.get(&resolved).header("Accept", "text/css,*/*;q=0.1").send() {
                if css_resp.status().is_success() {
                    if let Ok(css_text) = css_resp.text() {
                        let nested_imports = extract_css_imports(&css_text);
                        for imp in nested_imports {
                            let imp_res = resolve_relative_url(&resolved, &imp);
                            if imp_res.starts_with("http://") || imp_res.starts_with("https://") {
                                if let Ok(n_resp) = client.get(&imp_res).header("Accept", "text/css,*/*;q=0.1").send() {
                                    if n_resp.status().is_success() {
                                        if let Ok(n_text) = n_resp.text() {
                                            external_css.push(n_text);
                                        }
                                    }
                                }
                            }
                        }
                        external_css.push(css_text);
                    }
                }
            }
        }
    }

    Ok(PageResponse {
        url: final_url,
        html: final_html,
        external_css,
        title,
        status,
        status_text,
        headers,
        duration_ms,
    })
}

/// Decode a response body using the charset from the HTTP header, an HTML
/// `<meta charset>` declaration, or UTF-8 as a last resort.
pub fn decode_body(bytes: &[u8], content_type: Option<&str>) -> String {
    if let Some(label) = content_type.and_then(charset_from_content_type) {
        if let Some(decoded) = decode_with_label(bytes, &label) {
            return decoded;
        }
    }
    if let Some(label) = sniff_meta_charset(bytes) {
        if let Some(decoded) = decode_with_label(bytes, &label) {
            return decoded;
        }
    }
    String::from_utf8_lossy(bytes).into_owned()
}

fn charset_from_content_type(content_type: &str) -> Option<String> {
    content_type
        .to_ascii_lowercase()
        .split(';')
        .find_map(|part| part.trim().strip_prefix("charset=").map(|c| c.trim_matches('"').to_string()))
}

/// Looks for `<meta charset=...>` or `<meta http-equiv=content-type; charset=...>`
/// in the leading bytes, where such declarations must appear to be honoured.
fn sniff_meta_charset(bytes: &[u8]) -> Option<String> {
    let head = &bytes[..bytes.len().min(2048)];
    let text = String::from_utf8_lossy(head).to_ascii_lowercase();
    let mut search_from = 0;
    while let Some(rel) = text[search_from..].find("<meta") {
        let start = search_from + rel;
        let end = text[start..].find('>').map(|r| start + r + 1).unwrap_or(text.len());
        let tag = &text[start..end];
        if let Some(idx) = tag.find("charset") {
            let rest = tag[idx + 7..].trim_start_matches([' ', '=', '"', '\'']);
            let label: String = rest
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | ':'))
                .collect();
            if !label.is_empty() {
                return Some(label);
            }
        }
        search_from = end;
    }
    None
}

/// Latin-1 / windows-1252 byte-to-codepoint tables differ only in 0x80..0x9F;
/// the printable half is identical to Unicode, so a 128-entry table suffices.
const WINDOWS_1252_HIGH: [char; 32] = [
    '\u{20AC}', '\u{FFFD}', '\u{201A}', '\u{0192}', '\u{201E}', '\u{2026}', '\u{2020}', '\u{2021}',
    '\u{02C6}', '\u{2030}', '\u{0160}', '\u{2039}', '\u{0152}', '\u{FFFD}', '\u{017D}', '\u{FFFD}',
    '\u{FFFD}', '\u{2018}', '\u{2019}', '\u{201C}', '\u{201D}', '\u{2022}', '\u{2013}', '\u{2014}',
    '\u{02DC}', '\u{2122}', '\u{0161}', '\u{203A}', '\u{0153}', '\u{FFFD}', '\u{017E}', '\u{0178}',
];

fn byte_to_char(b: u8, windows_1252: bool) -> char {
    if b < 0x80 {
        b as char
    } else if windows_1252 && (b as usize) < 0xA0 {
        WINDOWS_1252_HIGH[b as usize - 0x80]
    } else {
        b as char
    }
}

fn decode_with_label(bytes: &[u8], label: &str) -> Option<String> {
    let norm = label.trim().to_ascii_lowercase().replace(['_', ' '], "-");
    match norm.as_str() {
        "utf-8" | "utf8" => Some(
            String::from_utf8(bytes.to_vec())
                .unwrap_or_else(|e| String::from_utf8_lossy(e.as_bytes()).into_owned()),
        ),
        "iso-8859-1" | "latin1" => Some(bytes.iter().map(|&b| byte_to_char(b, false)).collect()),
        "windows-1252" | "win-1252" | "cp1252" => {
            Some(bytes.iter().map(|&b| byte_to_char(b, true)).collect())
        }
        "us-ascii" | "ascii" => Some(String::from_utf8_lossy(bytes).into_owned()),
        _ => None,
    }
}

/// Length of the text a no-JS renderer can actually show: script/style/template
/// payloads and comments are dropped, then tags are stripped and whitespace
/// collapsed. Used to detect pages whose content only exists after JS runs.
pub fn visible_text_len(html: &str) -> usize {
    let without_comments = strip_comments(html);
    let without_scripts = strip_element_bodies(&without_comments, &["script", "style", "template", "noscript"]);
    let without_tags = strip_tags(&without_scripts);
    without_tags.split_whitespace().map(|w| w.chars().count()).sum()
}

fn strip_comments(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut rest = html;
    while let Some(start) = rest.find("<!--") {
        out.push_str(&rest[..start]);
        let end = rest[start..]
            .find("-->")
            .map(|r| start + r + 3)
            .unwrap_or(rest.len());
        rest = &rest[end..];
    }
    out.push_str(rest);
    out
}

fn strip_element_bodies(html: &str, tags: &[&str]) -> String {
    let mut out = String::with_capacity(html.len());
    let lower = html.to_ascii_lowercase();
    let mut pos = 0;
    while pos < html.len() {
        let mut nearest: Option<(usize, &str)> = None;
        for tag in tags {
            let open = format!("<{}", tag);
            if let Some(rel) = lower[pos..].find(&open) {
                let abs = pos + rel;
                let followed_by_boundary = lower[abs + open.len()..]
                    .chars()
                    .next()
                    .map_or(true, |c| c == '>' || c.is_whitespace() || c == '/');
                if followed_by_boundary && nearest.map_or(true, |(n, _)| abs < n) {
                    nearest = Some((abs, *tag));
                }
            }
        }
        let Some((start, tag)) = nearest else {
            out.push_str(&html[pos..]);
            break;
        };
        out.push_str(&html[pos..start]);
        let open_end = lower[start..].find('>').map(|r| start + r + 1).unwrap_or(html.len());
        let close = format!("</{}>", tag);
        let resume = lower[open_end..]
            .find(&close)
            .map(|r| open_end + r + close.len())
            .unwrap_or(html.len());
        pos = resume.min(html.len());
    }
    out
}

fn strip_tags(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut in_tag = false;
    for c in html.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    out
}

/// Notice shown instead of a JS-only page, whose static HTML has no content.
fn spa_notice_html(url: &str, title: &str) -> String {
    let host = host_of(url);
    let safe_title = title.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;");
    format!(
        "<html><head><title>{safe_title}</title></head><body style='background-color: #202124; color: #e8eaed; font-family: sans-serif; margin: 0;'><div style='max-width: 560px; margin: 64px 0 0 64px;'><h1 style='font-size: 22px; color: #e8eaed; margin: 0 0 16px 0;'>{host} needs JavaScript to show its content</h1><p style='font-size: 14px; color: #9aa0a6; margin: 0 0 12px 0;'>Aether renders plain HTML and CSS with no JavaScript engine, so this page's text never arrives — scripts fetch it after load. Only empty placeholders would be visible.</p><p style='font-size: 13px; color: #9aa0a6; margin: 0;'>Open the original page in a JavaScript-capable browser: <a href='{url}' style='color: #8ab4f8;'>{url}</a></p></div></body></html>"
    )
}

/// Sites that build themselves in JavaScript still ship their real content as
/// a JSON blob in the first response. Reading that blob shows true page data;
/// nothing here is invented.
pub fn embedded_json_content_html(url: &str, html: &str) -> Option<String> {
    if !url.contains("youtube.com") {
        return None;
    }
    let (page_title, content) = if url.contains("/watch") {
        watch_page_content(html)?
    } else {
        feed_page_content(url, html)?
    };
    let banner = format!(
        "<div style=\"background:#e8f0fe;color:#11274f;padding:10px 16px;font-size:13px\">Real page data embedded by YouTube in its first response. The player needs JavaScript, so to watch: <a href=\"{url}\">{url}</a></div>"
    );
    Some(format!(
        "<html><head><title>{}</title></head><body>{banner}{}</body></html>",
        escape_html_text(&page_title),
        content
    ))
}

/// The JSON object that starts just after an assignment such as
/// `var ytInitialData =`. Braces inside strings are ignored.
fn extract_json_object_after(html: &str, marker: &str) -> Option<serde_json::Value> {
    let start = html.find(marker)? + marker.len();
    let rest = &html[start..];
    let open = rest.find('{')?;
    let bytes = rest.as_bytes();
    let mut depth = 0i32;
    let mut in_string = false;
    let mut escaped = false;
    for (offset, byte) in bytes[open..].iter().enumerate() {
        if in_string {
            if escaped {
                escaped = false;
            } else if *byte == b'\\' {
                escaped = true;
            } else if *byte == b'"' {
                in_string = false;
            }
            continue;
        }
        match *byte {
            b'"' => in_string = true,
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    let slice = &rest[open..open + offset + 1];
                    return serde_json::from_str(slice).ok();
                }
            }
            _ => {}
        }
    }
    None
}

fn json_str(value: &serde_json::Value, path: &str) -> String {
    value
        .get(path)
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string()
}

fn runs_text(value: &serde_json::Value) -> String {
    value
        .get("runs")
        .and_then(|runs| runs.get(0))
        .and_then(|run| run.get("text"))
        .and_then(|text| text.as_str())
        .unwrap_or_default()
        .to_string()
}

fn watch_page_content(html: &str) -> Option<(String, String)> {
    let player = extract_json_object_after(html, "var ytInitialPlayerResponse")?;
    let details = player.get("videoDetails")?;
    let title = json_str(details, "title");
    if title.trim().is_empty() {
        return None;
    }
    let author = json_str(details, "author");
    let views = json_str(details, "viewCount");
    let channel = json_str(details, "channelId");
    let seconds: u64 = json_str(details, "lengthSeconds").parse().unwrap_or(0);
    let description = json_str(details, "shortDescription");
    let author_link = if channel.is_empty() {
        escape_html_text(&author)
    } else {
        format!(
            "<a href=\"https://www.youtube.com/channel/{channel}\">{}</a>",
            escape_html_text(&author)
        )
    };
    let body = description
        .lines()
        .map(escape_html_text)
        .collect::<Vec<String>>()
        .join("<br>");
    let content = format!(
        "<div style=\"max-width:760px;margin:24px auto;font-family:sans-serif\"><h1>{}</h1><p>{} · {} views · {} min</p><div>{}</div></div>",
        escape_html_text(&title),
        author_link,
        views,
        seconds / 60,
        body
    );
    Some((title, content))
}

fn feed_page_content(url: &str, html: &str) -> Option<(String, String)> {
    let data = extract_json_object_after(html, "var ytInitialData")?;
    let mut entries: Vec<(String, String, String)> = Vec::new();
    collect_video_renderers(&data, 0, &mut entries);
    if entries.is_empty() {
        return None;
    }
    let heading = match extract_query_param(url, "search_query") {
        Some(q) if !q.trim().is_empty() => format!("{q} - YouTube search results"),
        _ => "YouTube".to_string(),
    };
    let items = entries
        .iter()
        .map(|(video_id, title, meta)| {
            format!(
                "<li style=\"margin:0 0 14px 0\"><a href=\"https://www.youtube.com/watch?v={video_id}\">{}</a><div style=\"font-size:13px;color:#606060\">{}</div></li>",
                escape_html_text(title),
                escape_html_text(meta)
            )
        })
        .collect::<Vec<String>>()
        .join("");
    Some((
        heading.clone(),
        format!(
            "<div style=\"max-width:760px;margin:24px auto;font-family:sans-serif\"><h1>{}</h1><ul>{items}</ul></div>",
            escape_html_text(&heading)
        ),
    ))
}

fn collect_video_renderers(
    value: &serde_json::Value,
    depth: u32,
    out: &mut Vec<(String, String, String)>,
) {
    if depth > 40 || out.len() >= 30 {
        return;
    }
    match value {
        serde_json::Value::Object(map) => {
            if let Some(video) = map.get("videoRenderer") {
                let title = runs_text(video.get("title").unwrap_or(&serde_json::Value::Null));
                let video_id = json_str(video, "videoId");
                if !title.trim().is_empty() && !video_id.is_empty() {
                    let owner = runs_text(
                        video
                            .get("ownerText")
                            .unwrap_or(&serde_json::Value::Null),
                    );
                    let published = json_str(
                        video
                            .get("publishedTimeText")
                            .unwrap_or(&serde_json::Value::Null),
                        "simpleText",
                    );
                    out.push((video_id, title, format!("{owner} {published}").trim().to_string()));
                }
            }
            for child in map.values() {
                collect_video_renderers(child, depth + 1, out);
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                collect_video_renderers(item, depth + 1, out);
            }
        }
        _ => {}
    }
}

fn extract_css_imports(css: &str) -> Vec<String> {
    let mut imports = Vec::new();
    let mut search_pos = 0;
    let lower = css.to_ascii_lowercase();
    while let Some(idx) = lower[search_pos..].find("@import") {
        let start = search_pos + idx + 7;
        let end = match css[start..].find(';') {
            Some(i) => start + i,
            None => break,
        };
        let slice = css[start..end].trim();
        let url = if slice.starts_with("url(") && slice.ends_with(')') {
            let inner = slice[4..slice.len() - 1].trim();
            inner.trim_matches('"').trim_matches('\'').to_string()
        } else {
            slice.trim_matches('"').trim_matches('\'').to_string()
        };
        if !url.is_empty() {
            imports.push(url);
        }
        search_pos = end + 1;
    }
    imports
}

fn extract_stylesheet_urls(html: &str) -> Vec<String> {
    let mut urls = Vec::new();
    let lower = html.to_ascii_lowercase();
    let mut search_pos = 0;
    while let Some(link_idx) = lower[search_pos..].find("<link") {
        let abs_start = search_pos + link_idx;
        let end_idx = match lower[abs_start..].find('>') {
            Some(i) => abs_start + i,
            None => break,
        };
        let tag_slice = &html[abs_start..=end_idx];

        let mut tag_for_parser = tag_slice.to_string();
        if !tag_for_parser.ends_with("/>") && tag_for_parser.ends_with('>') {
            tag_for_parser.insert(tag_for_parser.len() - 1, '/');
        }
        let parsed = crate::html_parser::HtmlParser::parse(&tag_for_parser);
        if let Some(link_node) = parsed.children.first() {
            let rel = link_node.get_attribute("rel").map(|s| s.to_ascii_lowercase()).unwrap_or_default();
            let as_attr = link_node.get_attribute("as").map(|s| s.to_ascii_lowercase()).unwrap_or_default();
            let media = link_node.get_attribute("media").map(|s| s.to_ascii_lowercase()).unwrap_or_default();

            // Ignore print-only stylesheets
            let is_print_only = media.contains("print") && !media.contains("screen") && !media.contains("all");
            let is_stylesheet = rel.split_whitespace().any(|r| r == "stylesheet")
                || (rel.split_whitespace().any(|r| r == "preload") && as_attr == "style");

            if is_stylesheet && !is_print_only {
                if let Some(href_val) = link_node.get_attribute("href") {
                    if !href_val.is_empty() {
                        let decoded = crate::html_parser::HtmlParser::decode_string_entities(href_val);
                        urls.push(decoded);
                    }
                }
            }
        }
        search_pos = end_idx + 1;
    }
    urls
}

fn extract_title_or(html: &str, default_title: &str) -> String {
    let lower = html.to_ascii_lowercase();
    if let Some(start) = lower.find("<title>") {
        if let Some(end) = lower[start..].find("</title>") {
            let title_content = &html[start + 7..start + end];
            let cleaned: String = title_content
                .chars()
                .filter(|c| {
                    !c.is_control()
                        && !matches!(
                            *c,
                            '\u{200B}'
                                | '\u{200C}'
                                | '\u{200D}'
                                | '\u{200E}'
                                | '\u{200F}'
                                | '\u{202A}'..='\u{202E}'
                                | '\u{2060}'..='\u{206F}'
                                | '\u{FEFF}'
                                | '\u{00AD}'
                        )
                })
                .collect();
            let trimmed = cleaned.trim();
            if !trimmed.is_empty() {
                return trimmed.to_string();
            }
        }
    }
    default_title.to_string()
}

fn extract_query_param(url_str: &str, param: &str) -> Option<String> {
    if let Ok(parsed) = Url::parse(url_str) {
        for (k, v) in parsed.query_pairs() {
            if k == param {
                return Some(v.into_owned());
            }
        }
    }
    None
}

/// A search results page, whichever engine produced it.
pub fn is_search_results_url(url_str: &str) -> bool {
    const RESULTS_PATHS: [&str; 8] = [
        "google.com/search",
        "bing.com/search",
        "duckduckgo.com/html",
        "duckduckgo.com/lite",
        "search.yahoo.com/search",
        "ecosia.org/search",
        "mojeek.com/search",
        "brave.com/search",
    ];
    let lower = url_str.to_ascii_lowercase();
    RESULTS_PATHS.iter().any(|path| lower.contains(path))
}

/// Links that point at the indexed web instead of at the engine itself. A
/// response with none of these carries no results, whatever its status code.
pub fn organic_result_links(html: &str) -> usize {
    const ENGINE_HOSTS: [&str; 9] = [
        "google.com",
        "googleusercontent.com",
        "gstatic.com",
        "bing.com",
        "duckduckgo.com",
        "yahoo.com",
        "ecosia.org",
        "mojeek.com",
        "brave.com",
    ];
    let mut count = 0usize;
    let mut rest = html;
    while let Some(start) = rest.find("href=\"http") {
        let after = &rest[start + 6..];
        let Some(end) = after.find('"') else { break };
        let href = &after[..end];
        if !ENGINE_HOSTS.iter().any(|host| href.contains(host)) {
            count += 1;
        }
        rest = &after[end..];
    }
    count
}

const MIN_ORGANIC_RESULTS: usize = 3;

/// Search engines reject an address that claims to be Chrome but arrives with
/// a Rust TLS fingerprint, and accept one that says what it is. Being truthful
/// is also what these plain-HTML endpoints are built for.
fn plain_reader_client() -> reqwest::blocking::Client {
    static CLIENT: std::sync::OnceLock<reqwest::blocking::Client> = std::sync::OnceLock::new();
    CLIENT
        .get_or_init(|| {
            reqwest::blocking::Client::builder()
                .user_agent("Aether/1.0 (scriptless HTML reader)")
                .timeout(std::time::Duration::from_secs(12))
                .build()
                .expect("client with only defaults builds")
        })
        .clone()
}

/// Keeps the engine's own answer when it contains results; otherwise replays
/// the query against plain-HTML engines and returns their live page.
fn live_search_body(url_str: &str, body: String) -> (String, String) {
    if organic_result_links(&body) >= MIN_ORGANIC_RESULTS {
        return (url_str.to_string(), body);
    }
    let Some(query) = extract_query_param(url_str, "q") else {
        return (url_str.to_string(), body);
    };
    let encoded = url::form_urlencoded::byte_serialize(query.as_bytes()).collect::<String>();
    let fallbacks = [
        // Brave still serves a full static results page to a scriptless client;
        // DuckDuckGo's html/ and lite/ endpoints are built for the same job but
        // challenge repeated addresses more eagerly.
        ("Brave", format!("https://search.brave.com/search?q={encoded}&source=web")),
        ("DuckDuckGo", format!("https://html.duckduckgo.com/html/?q={encoded}")),
        ("DuckDuckGo Lite", format!("https://lite.duckduckgo.com/lite/?q={encoded}")),
        ("Bing", format!("https://www.bing.com/search?q={encoded}&count=20")),
        ("Mojeek", format!("https://www.mojeek.com/search?q={encoded}")),
        ("Ecosia", format!("https://www.ecosia.org/search?q={encoded}")),
    ];
    // Whichever engine answered last time is tried first now, so an ordinary
    // search costs two requests instead of six. Fewer requests is the difference
    // between being treated as a browser and being throttled as a scraper.
    let start = LAST_ANSWERING_ENGINE
        .load(std::sync::atomic::Ordering::Relaxed)
        .min(fallbacks.len() - 1);
    let reader = plain_reader_client();
    for step in 0..fallbacks.len() {
        let index = (start + step) % fallbacks.len();
        let (engine, fallback_url) = (&fallbacks[index].0, &fallbacks[index].1);
        let Ok(response) = reader
            .get(fallback_url)
            .header("Accept", "text/html,application/xhtml+xml;q=0.9,*/*;q=0.8")
            .header("Accept-Language", "en-US,en;q=0.9")
            .send()
        else {
            continue;
        };
        let Ok(text) = response.text() else { continue };
        if organic_result_links(&text) >= MIN_ORGANIC_RESULTS {
            LAST_ANSWERING_ENGINE.store(index, std::sync::atomic::Ordering::Relaxed);
            let notice = search_source_notice(&query, engine, fallback_url);
            return (fallback_url.clone(), inject_after_opening_body(&text, &notice));
        }
    }
    (url_str.to_string(), search_unanswered_html(&query, url_str))
}

static LAST_ANSWERING_ENGINE: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);

/// No plain-HTML engine answered this time. Say so plainly rather than leaving
/// the user with an empty shell that looks like a broken renderer.
fn search_unanswered_html(query: &str, url_str: &str) -> String {
    format!(
        "<html><head><title>No results for {q}</title></head><body><div style=\"max-width:640px;margin:32px auto;font-family:sans-serif\"><h1>No results for {q}</h1><p style=\"font-size:14px\">Google answered with a page that needs JavaScript, and Brave, Bing, DuckDuckGo, DuckDuckGo Lite, Mojeek and Ecosia returned nothing usable for this query just now. Aether only paints results that a live engine actually sent; it will not fill the page in on your behalf.</p><p style=\"font-size:14px\">Try the search again, or open the original page: <a href=\"{u}\">{u}</a></p></div></body></html>",
        q = escape_html_text(query),
        u = escape_html_text(url_str)
    )
}

/// Banner naming the engine that actually answered, so the page never looks
/// like a locally invented results list.
pub fn search_source_notice(query: &str, engine: &str, engine_url: &str) -> String {
    format!(
        "<div style=\"background:#fef7e0;color:#5f4b00;padding:10px 16px;font-size:13px\">Google returned a page that needs JavaScript. These are live results from <a href=\"{engine_url}\">{engine}</a> for \u{201c}{}\u{201d}.</div>",
        escape_html_text(query)
    )
}

fn escape_html_text(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(ch),
        }
    }
    out
}

/// Drops a fragment in just inside `<body>`, falling back to a prepended block.
pub fn inject_after_opening_body(html: &str, fragment: &str) -> String {
    let lower = html.to_ascii_lowercase();
    if let Some(tag_start) = lower.find("<body") {
        if let Some(rel_end) = lower[tag_start..].find('>') {
            let insert_at = tag_start + rel_end + 1;
            let mut out = String::with_capacity(html.len() + fragment.len());
            out.push_str(&html[..insert_at]);
            out.push_str(fragment);
            out.push_str(&html[insert_at..]);
            return out;
        }
    }
    format!("{fragment}{html}")
}

pub const AI_MODE_HTML: &str = r#"
<!DOCTYPE html>
<html>
<head>
    <title>Aether AI Mode</title>
    <style>
        body {
            font-family: sans-serif;
            margin: 0;
            padding: 24px 32px;
            background-color: #202124;
            color: #bdc1c6;
        }
        .container {
            max-width: 820px;
            margin: 0 auto;
        }
        .header {
            text-align: center;
            padding: 24px 0;
        }
        .sparkle {
            font-size: 32px;
            color: #8ab4f8;
            margin-bottom: 8px;
        }
        h1 {
            color: #ffffff;
            font-size: 34px;
            margin: 0 0 8px 0;
        }
        .subtitle {
            color: #9aa0a6;
            font-size: 15px;
            margin: 0 0 24px 0;
        }
        .card {
            background-color: #303134;
            border: 1px solid #3c4043;
            border-radius: 12px;
            padding: 20px 24px;
            margin-bottom: 20px;
        }
        .card h2 {
            color: #8ab4f8;
            font-size: 20px;
            margin-top: 0;
            margin-bottom: 8px;
        }
        .card p {
            font-size: 14px;
            line-height: 1.6;
            color: #e8eaed;
            margin: 0 0 12px 0;
        }
        .code-box {
            background-color: #202124;
            border: 1px solid #444746;
            border-radius: 6px;
            padding: 12px;
            font-family: monospace;
            font-size: 13px;
            color: #a8c7fa;
            margin-bottom: 14px;
        }
        .pills {
            margin-top: 12px;
        }
        .pill {
            display: inline-block;
            background-color: #3c4043;
            color: #8ab4f8;
            padding: 6px 14px;
            border-radius: 14px;
            text-decoration: none;
            font-size: 12px;
            font-weight: bold;
            margin-right: 8px;
            margin-bottom: 8px;
            border: 1px solid #5f6368;
        }
        .footer-nav {
            text-align: center;
            margin-top: 32px;
            color: #9aa0a6;
            font-size: 13px;
        }
        .footer-nav a {
            color: #8ab4f8;
            text-decoration: none;
            margin: 0 8px;
        }
    </style>
</head>
<body>
    <div class="container">
        <div class="header">
            <div class="sparkle">✦</div>
            <h1>Aether AI Assistant</h1>
            <p class="subtitle">Autonomous Intelligence & Knowledge Engine • Pure Rust Web Architecture</p>
        </div>

        <div class="card">
            <h2>🦀 Pure Rust Engine Architecture</h2>
            <p>Aether is designed from the ground up without Chromium, WebKit, or Blink. Every component from network fetch to visual rendering is compiled with Rust's strict memory safety and fearless concurrency.</p>
            <div class="code-box">HTTP/S (Rustls) ➔ HTML Tokenizer ➔ CSS Specificity Cascade ➔ Box Model (Flow) ➔ Painter</div>
            <div class="pills">
                <a href="about:credits" class="pill">View Architecture Specification</a>
                <a href="about:test" class="pill">Run CSS Layout Test Suite</a>
            </div>
        </div>

        <div class="card">
            <h2>⚡ Instant Encyclopedia & Web Research</h2>
            <p>Search any query or topic from the Omnibox. Results connect directly to open encyclopedia resources and synthesized Wikipedia articles without tracking.</p>
            <div class="pills">
                <a href="https://search.aether.local/?q=Rust+programming+language" class="pill">Research: Rust Programming Language</a>
                <a href="https://search.aether.local/?q=Quantum+computing" class="pill">Research: Quantum Computing</a>
                <a href="https://search.aether.local/?q=Compiler+design" class="pill">Research: Compiler Design</a>
            </div>
        </div>

        <div class="card">
            <h2>🛡️ Zero Tracking & Privacy Focused</h2>
            <p>No telemetry, no background service workers, and no proprietary ad networks. Your browsing session remains strictly between your device and the destination server.</p>
        </div>

        <div class="footer-nav">
            <a href="about:newtab">&larr; Return to New Tab</a> &bull;
            <a href="about:credits">System Diagnostics</a> &bull;
            <a href="about:test">CSS Test Suite</a>
        </div>
    </div>
</body>
</html>
"#;

pub const WELCOME_HTML: &str = r#"
<!DOCTYPE html>
<html>
<head>
    <title>Aether - 100% Pure Rust Web Browser</title>
    <style>
        body {
            font-family: sans-serif;
            margin: 0;
            padding: 32px;
            background-color: #f8fafc;
            color: #1e293b;
        }
        .hero {
            background-color: #ffffff;
            padding: 32px;
            border-radius: 12px;
            border: 1px solid #e2e8f0;
            margin-bottom: 24px;
        }
        h1 {
            color: #0f172a;
            font-size: 32px;
            margin-top: 0;
            margin-bottom: 8px;
        }
        .tagline {
            color: #64748b;
            font-size: 18px;
            margin-bottom: 20px;
        }
        .badge {
            display: inline-block;
            background-color: #e0f2fe;
            color: #0369a1;
            padding: 4px 10px;
            border-radius: 16px;
            font-size: 13px;
            font-weight: bold;
            margin-right: 8px;
        }
        .badge-rust {
            background-color: #ffedd5;
            color: #c2410c;
        }
        .grid {
            margin-top: 24px;
        }
        .card {
            background-color: #ffffff;
            padding: 20px;
            border-radius: 8px;
            border: 1px solid #e2e8f0;
            margin-bottom: 16px;
        }
        h2 {
            color: #334155;
            font-size: 20px;
            margin-top: 0;
            margin-bottom: 10px;
        }
        p {
            color: #475569;
            line-height: 1.6;
        }
        .btn {
            display: inline-block;
            background-color: #2563eb;
            color: #ffffff;
            padding: 8px 18px;
            border-radius: 6px;
            text-decoration: none;
            font-weight: bold;
            margin-top: 8px;
            margin-right: 12px;
        }
        .btn-outline {
            background-color: #ffffff;
            color: #2563eb;
            border: 1px solid #2563eb;
        }
        ul {
            padding-left: 20px;
        }
        li {
            margin-bottom: 6px;
            color: #475569;
        }
        .footer {
            margin-top: 32px;
            text-align: center;
            color: #94a3b8;
            font-size: 14px;
        }
    </style>
</head>
<body>
    <div class="hero">
        <span class="badge badge-rust">Pure Rust Engine</span>
        <span class="badge">Zero Chromium</span>
        <span class="badge">Fast & Memory Safe</span>
        <h1>Welcome to Aether</h1>
        <p class="tagline">An independent, lightweight web browser built 100% in Rust from scratch.</p>
        <p>Aether features a complete standard browser pipeline: from secure HTTP/S network fetching, HTML tokenizer and DOM tree generator, to CSS cascade with specificity calculation, Box Model layout engine, and built-in interactive DevTools.</p>
        <div>
            <a href="about:test" class="btn">Run CSS Test Suite</a>
            <a href="about:credits" class="btn btn-outline">Engine Architecture</a>
            <a href="about:ai" class="btn btn-outline">Aether AI Mode</a>
        </div>
    </div>

    <div class="grid">
        <div class="card">
            <h2>Zero-Chromium Independent Architecture</h2>
            <ul>
                <li><strong>HTML Tokenizer & DOM:</strong> Parses standard HTML tags, attributes, void elements, and constructs the DOM tree.</li>
                <li><strong>CSS Cascade & Specificity:</strong> Evaluates selectors (class, id, tag, combinators) and calculates W3C specificity scores.</li>
                <li><strong>CSS Box Model:</strong> Resolves Margin, Border, Padding, Width, Height, and word-wrapping layout flow.</li>
                <li><strong>DevTools Inspector (F12):</strong> Live inspection of DOM hierarchy, computed styles, and interactive Box Model diagrams.</li>
            </ul>
        </div>

        <div class="card">
            <h2>Explore Navigation</h2>
            <p>You can type any web URL in the Omnibox above (e.g. <code>https://example.com</code> or <code>about:test</code>) and press Enter, or click the links below:</p>
            <ul>
                <li><a href="about:test">CSS Layout & UI Test Suite (about:test)</a></li>
                <li><a href="about:credits">Engine Architecture & Credits (about:credits)</a></li>
                <li><a href="about:ai">Aether AI Mode (about:ai)</a></li>
            </ul>
        </div>
    </div>

    <div class="footer">
        <p>Aether Browser Engine &bull; Pure Rust &bull; Independent from Chromium and Blink</p>
    </div>
</body>
</html>
"#;

pub const TEST_SUITE_HTML: &str = r#"
<!DOCTYPE html>
<html>
<head>
    <title>CSS & Layout Test Suite - Aether</title>
    <style>
        body {
            font-family: sans-serif;
            background-color: #f1f5f9;
            color: #0f172a;
            margin: 0;
            padding: 24px;
        }
        .container {
            background-color: #ffffff;
            padding: 24px;
            border-radius: 8px;
            border: 1px solid #cbd5e1;
            margin-bottom: 20px;
        }
        h1 {
            color: #1e3a8a;
            border-bottom: 2px solid #3b82f6;
            padding-bottom: 8px;
            margin-top: 0;
        }
        h2 {
            color: #1d4ed8;
            margin-top: 16px;
        }
        .box-test {
            margin: 16px 0;
            padding: 16px;
            background-color: #e0e7ff;
            border: 2px solid #6366f1;
            border-radius: 6px;
        }
        .highlight {
            background-color: #fef08a;
            color: #854d0e;
            padding: 2px 6px;
            border-radius: 4px;
            font-weight: bold;
        }
        .card-row {
            margin-top: 16px;
        }
        .card-green {
            background-color: #dcfce7;
            border: 1px solid #86efac;
            padding: 12px;
            border-radius: 6px;
            margin-bottom: 10px;
        }
        .card-red {
            background-color: #fee2e2;
            border: 1px solid #fca5a5;
            padding: 12px;
            border-radius: 6px;
            margin-bottom: 10px;
        }
        .card-blue {
            background-color: #dbeafe;
            border: 1px solid #93c5fd;
            padding: 12px;
            border-radius: 6px;
            margin-bottom: 10px;
        }
        blockquote {
            background-color: #f8fafc;
            border-left: 4px solid #3b82f6;
            padding: 10px 16px;
            margin: 14px 0;
        }
        code {
            background-color: #f1f5f9;
            color: #dc2626;
            padding: 2px 4px;
            border-radius: 3px;
        }
        pre {
            background-color: #1e293b;
            color: #f8fafc;
            padding: 14px;
            border-radius: 6px;
        }
        .nav-links {
            margin-top: 20px;
        }
        .nav-links a {
            margin-right: 14px;
            font-weight: bold;
        }
    </style>
</head>
<body>
    <div class="container">
        <h1>CSS & Layout Verification Suite</h1>
        <p>This page validates the accuracy of the CSS parser, selector matcher, and box model layout engine.</p>

        <div class="box-test">
            <h3>Box Model: Margin, Border, Padding</h3>
            <p>This container tests inner padding, a purple solid border, and outer margin flow. Inside is <span class="highlight">highlighted inline text</span>.</p>
        </div>

        <h2>Color and Block Box Verification</h2>
        <div class="card-row">
            <div class="card-green">
                <strong>Green Card (Success Box):</strong> Validates RGB background fills, corner radius, and bold typography.
            </div>
            <div class="card-blue">
                <strong>Blue Card (Info Box):</strong> Validates text color inheritance and fluid block width.
            </div>
            <div class="card-red">
                <strong>Red Card (Alert Box):</strong> Validates cascading border and background rules.
            </div>
        </div>

        <h2>Blockquote and Code Blocks</h2>
        <blockquote>
            "The mission is to build a modern, high-performance web browser completely from scratch in safe Rust with zero Chromium dependencies."
        </blockquote>

        <p>The code block below is styled with a <code>&lt;pre&gt;</code> element:</p>
        <pre>fn main() {
    println!("Hello from Pure Rust Browser!");
    let dom = HtmlParser::parse("&lt;h1&gt;Aether&lt;/h1&gt;");
    let layout = build_layout_tree(&dom);
}</pre>

        <div class="nav-links">
            <a href="about:welcome">&larr; Return to Welcome</a>
            <a href="about:credits">View Engine Architecture &rarr;</a>
        </div>
    </div>
</body>
</html>
"#;

pub const CREDITS_HTML: &str = r#"
<!DOCTYPE html>
<html>
<head>
    <title>Aether Engine Architecture & Credits</title>
    <style>
        body {
            font-family: sans-serif;
            background-color: #0f172a;
            color: #e2e8f0;
            padding: 32px;
            margin: 0;
        }
        .wrapper {
            background-color: #1e293b;
            padding: 28px;
            border-radius: 10px;
            border: 1px solid #334155;
        }
        h1 {
            color: #38bdf8;
            margin-top: 0;
        }
        h2 {
            color: #7dd3fc;
            margin-top: 24px;
        }
        .pipeline {
            background-color: #0b1120;
            padding: 16px;
            border-radius: 8px;
            border: 1px solid #1e293b;
            margin: 16px 0;
        }
        .step {
            margin-bottom: 12px;
            padding: 8px 12px;
            background-color: #1e293b;
            border-radius: 4px;
            border-left: 3px solid #38bdf8;
        }
        a {
            color: #38bdf8;
        }
    </style>
</head>
<body>
    <div class="wrapper">
        <h1>Aether Browser Engine Architecture</h1>
        <p>Aether proves that a fast, clean, memory-safe web browser can be engineered purely in Rust without relying on heavy engines like Chromium, WebKit, or Blink.</p>

        <h2>Standard Browser Pipeline:</h2>
        <div class="pipeline">
            <div class="step">
                <strong>1. Network Fetcher (network.rs):</strong> Loads HTTP/HTTPS resources over secure TLS (Rustls), and handles internal protocols like about: and file://.
            </div>
            <div class="step">
                <strong>2. HTML Parser & DOM Builder (html_parser.rs):</strong> Tokenizes HTML tags, attributes, void elements, and builds the DOM Node tree.
            </div>
            <div class="step">
                <strong>3. CSS Parser & Cascading (css.rs, style.rs):</strong> Parses stylesheet rules, computes selector specificity, and resolves inherited styles.
            </div>
            <div class="step">
                <strong>4. Box Model Layout Engine (layout.rs):</strong> Computes Margin, Border, Padding, Content dimensions, auto centering, and word-wrapping flow.
            </div>
            <div class="step">
                <strong>5. Display List & Painter (render.rs):</strong> Generates optimized display lists, renders images, and handles interactive link hit-testing.
            </div>
            <div class="step">
                <strong>6. Interactive Shell & DevTools (app.rs):</strong> Multi-tab interface, custom borderless window controls, real-time search autocomplete, and F12 DevTools.
            </div>
        </div>

        <p><a href="about:welcome">&larr; Return to Welcome</a></p>
    </div>
</body>
</html>
"#;
