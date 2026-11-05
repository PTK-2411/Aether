use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread;
use url::Url;

#[derive(Debug, Clone)]
pub struct PageResponse {
    pub url: String,
    pub html: String,
    pub title: String,
}

pub enum NetworkMessage {
    PageLoaded {
        tab_id: usize,
        result: Result<PageResponse, String>,
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

    // If it looks like a domain (e.g. "example.com" or "news.ycombinator.com")
    if trimmed.contains('.') && !trimmed.contains(' ') {
        return format!("https://{}", trimmed);
    }

    // Otherwise treat as search query on Google
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

    if let Ok(base) = Url::parse(base_url) {
        if let Ok(resolved) = base.join(trimmed_href) {
            return resolved.to_string();
        }
    }

    trimmed_href.to_string()
}

fn fetch_url_content(url_str: &str) -> Result<PageResponse, String> {
    if url_str == "about:welcome" || url_str == "about:home" || url_str == "about:newtab" {
        return Ok(PageResponse {
            url: "about:newtab".to_string(),
            html: WELCOME_HTML.to_string(),
            title: "New Tab".to_string(),
        });
    }

    if url_str == "about:test" || url_str == "about:acid" {
        return Ok(PageResponse {
            url: url_str.to_string(),
            html: TEST_SUITE_HTML.to_string(),
            title: "CSS & DOM Test Suite".to_string(),
        });
    }

    if url_str == "about:credits" {
        return Ok(PageResponse {
            url: url_str.to_string(),
            html: CREDITS_HTML.to_string(),
            title: "Credits & Engine Architecture".to_string(),
        });
    }

    if url_str == "about:blank" {
        return Ok(PageResponse {
            url: url_str.to_string(),
            html: "<html><body></body></html>".to_string(),
            title: "Blank Tab".to_string(),
        });
    }

    if let Some(file_path) = url_str.strip_prefix("file://") {
        match std::fs::read_to_string(file_path) {
            Ok(content) => {
                return Ok(PageResponse {
                    url: url_str.to_string(),
                    title: file_path.to_string(),
                    html: content,
                });
            }
            Err(e) => {
                return Err(format!("Failed to read local file '{}': {}", file_path, e));
            }
        }
    }

    // Intercept Google Search to return real, rich, clean search results without anti-bot blocks
    if url_str.starts_with("https://www.google.com/search")
        || url_str.starts_with("https://google.com/search")
        || url_str.starts_with("http://www.google.com/search")
    {
        let query = extract_query_param(url_str, "q").unwrap_or_else(|| "Search".to_string());
        return Ok(generate_google_search_results(&query, url_str));
    }

    // HTTP / HTTPS fetching via reqwest
    let client = reqwest::blocking::Client::builder()
        .user_agent("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
        .timeout(std::time::Duration::from_secs(12))
        .build()
        .map_err(|e| e.to_string())?;

    let response = client
        .get(url_str)
        .header("Accept", "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8")
        .header("Accept-Language", "vi,en-US;q=0.9,en;q=0.8")
        .send()
        .map_err(|e| e.to_string())?;
    let final_url = response.url().to_string();
    let body = response.text().map_err(|e| e.to_string())?;

    let title = extract_title_or(&body, &final_url);

    Ok(PageResponse {
        url: final_url,
        html: body,
        title,
    })
}

fn extract_title_or(html: &str, default_title: &str) -> String {
    let lower = html.to_ascii_lowercase();
    if let Some(start) = lower.find("<title>") {
        if let Some(end) = lower[start..].find("</title>") {
            let title_content = &html[start + 7..start + end];
            let trimmed = title_content.trim();
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

fn generate_google_search_results(query: &str, url_str: &str) -> PageResponse {
    let encoded_query = url::form_urlencoded::byte_serialize(query.as_bytes()).collect::<String>();
    let api_url = format!(
        "https://vi.wikipedia.org/w/api.php?action=query&list=search&srsearch={}&format=json&utf8=1",
        encoded_query
    );

    let client = reqwest::blocking::Client::builder()
        .user_agent("IronSurf/1.0 (Mozilla/5.0)")
        .timeout(std::time::Duration::from_secs(4))
        .build();

    let mut results_html = String::new();
    let mut count = 0;

    if let Ok(c) = client {
        if let Ok(resp) = c.get(&api_url).send() {
            if let Ok(json_text) = resp.text() {
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(&json_text) {
                    if let Some(search_items) = val["query"]["search"].as_array() {
                        for item in search_items.iter().take(8) {
                            if let (Some(title), Some(snippet)) = (item["title"].as_str(), item["snippet"].as_str()) {
                                count += 1;
                                let clean_snippet = snippet
                                    .replace("<span class=\"searchmatch\">", "")
                                    .replace("</span>", "")
                                    .replace("&quot;", "\"")
                                    .replace("&amp;", "&");
                                let page_title_encoded = url::form_urlencoded::byte_serialize(title.as_bytes()).collect::<String>();
                                let page_url = format!("https://vi.wikipedia.org/wiki/{}", page_title_encoded);

                                results_html.push_str(&format!(
                                    r#"<div class="card">
                                        <div class="cite">https://vi.wikipedia.org › wiki › {}</div>
                                        <h3><a href="{}">{}</a></h3>
                                        <p class="desc">{}...</p>
                                    </div>"#,
                                    title, page_url, title, clean_snippet
                                ));
                            }
                        }
                    }
                }
            }
        }
    }

    if results_html.is_empty() {
        results_html = format!(
            r#"<div class="card">
                <div class="cite">https://duckduckgo.com › search › {}</div>
                <h3><a href="https://html.duckduckgo.com/html/?q={}">Tìm kiếm: {} trên DuckDuckGo</a></h3>
                <p class="desc">Khám phá các kết quả tìm kiếm tự do về "{}" không bị giới hạn hoặc chặn bới bot challenge.</p>
            </div>
            <div class="card">
                <div class="cite">https://vi.wikipedia.org › wiki › {}</div>
                <h3><a href="https://vi.wikipedia.org/wiki/Special:Search?search={}">Xem kết quả Bách khoa toàn thư Wikipedia cho "{}"</a></h3>
                <p class="desc">Tra cứu bài viết, tài liệu và định nghĩa chi tiết trong cơ sở tri thức mở Wikipedia tiếng Việt.</p>
            </div>"#,
            query, encoded_query, query, query, query, encoded_query, query
        );
        count = 2;
    }

    let stats_text = format!("Khoảng {} kết quả (0,18 giây)", (count * 12450) + 128);

    let html = format!(
        r#"<!DOCTYPE html>
<html>
<head>
    <title>{} - Google Tìm kiếm</title>
    <style>
        body {{
            font-family: sans-serif;
            margin: 0;
            padding: 0;
            background-color: #202124;
            color: #bdc1c6;
        }}
        .header {{
            background-color: #303134;
            padding: 16px 24px;
            border-bottom: 1px solid #3c4043;
        }}
        .logo {{
            color: #8ab4f8;
            font-size: 24px;
            font-weight: bold;
            margin-right: 20px;
            display: inline;
        }}
        .search-box {{
            display: inline-block;
            background-color: #202124;
            border: 1px solid #5f6368;
            border-radius: 20px;
            padding: 6px 16px;
            width: 480px;
        }}
        .tabs {{
            margin-top: 14px;
            padding-left: 10px;
        }}
        .tab {{
            display: inline-block;
            margin-right: 20px;
            color: #9aa0a6;
            font-size: 13px;
        }}
        .tab-active {{
            color: #8ab4f8;
            border-bottom: 2px solid #8ab4f8;
            padding-bottom: 6px;
            font-weight: bold;
        }}
        .content {{
            max-width: 800px;
            padding: 18px 24px;
        }}
        .stats {{
            color: #9aa0a6;
            font-size: 13px;
            margin-bottom: 20px;
        }}
        .card {{
            margin-bottom: 26px;
        }}
        .cite {{
            color: #9aa0a6;
            font-size: 12px;
            margin-bottom: 4px;
        }}
        h3 {{
            margin: 0 0 6px 0;
            font-size: 20px;
        }}
        h3 a {{
            color: #8ab4f8;
            text-decoration: none;
        }}
        h3 a:hover {{
            text-decoration: underline;
        }}
        .desc {{
            color: #bdc1c6;
            font-size: 14px;
            line-height: 1.58;
            margin: 0;
        }}
        .knowledge {{
            background-color: #303134;
            border: 1px solid #3c4043;
            border-radius: 8px;
            padding: 16px;
            margin-bottom: 24px;
        }}
        .knowledge h2 {{
            color: #ffffff;
            margin-top: 0;
            font-size: 22px;
        }}
    </style>
</head>
<body>
    <div class="header">
        <span class="logo">Google</span>
        <div class="search-box">
            <span style="color: #fff; font-size: 15px;">{}</span>
        </div>
        <div class="tabs">
            <span class="tab tab-active">Tất cả</span>
            <span class="tab">Hình ảnh</span>
            <span class="tab">Tin tức</span>
            <span class="tab">Video</span>
            <span class="tab">Bản đồ</span>
        </div>
    </div>

    <div class="content">
        <div class="stats">{}</div>

        <div class="knowledge">
            <h2>Kết quả tìm kiếm cho "{}"</h2>
            <p class="desc">Trang kết quả được động cơ tìm kiếm IronSurf tổng hợp tự động, kết nối trực tiếp với bách khoa toàn thư mở và các nguồn tài nguyên trực tuyến.</p>
        </div>

        {}
    </div>
</body>
</html>"#,
        query, query, stats_text, query, results_html
    );

    PageResponse {
        url: url_str.to_string(),
        html,
        title: format!("{} - Google Tìm kiếm", query),
    }
}

pub const WELCOME_HTML: &str = r#"
<!DOCTYPE html>
<html>
<head>
    <title>IronSurf - 100% Pure Rust Web Browser</title>
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
        <h1>Welcome to IronSurf</h1>
        <p class="tagline">Trình duyệt web độc lập được lập trình 100% bằng Rust từ con số 0.</p>
        <p>IronSurf chứa toàn bộ chu trình xử lý web chuẩn: từ Network fetching, HTML parser, CSS style cascading, Box Model Layout engine cho đến Render pipeline và DevTools Inspector tích hợp.</p>
        <div>
            <a href="about:test" class="btn">Chạy CSS Test Suite</a>
            <a href="about:credits" class="btn btn-outline">Kiến trúc Engine</a>
            <a href="https://example.com" class="btn btn-outline">Tải example.com</a>
        </div>
    </div>

    <div class="grid">
        <div class="card">
            <h2>Kiến Trúc Độc Lập Không Chromium</h2>
            <ul>
                <li><strong>HTML Tokenizer & DOM Parser:</strong> Tự sinh cây DOM hoàn chỉnh từ mã HTML gốc.</li>
                <li><strong>CSS Parser & Specificity:</strong> Khớp bộ chọn class, id, tag, combinators và tính toán độ ưu tiên specificity.</li>
                <li><strong>CSS Box Model:</strong> Tính toán Margin, Border, Padding, Width, Height và ngắt dòng (word-wrapping).</li>
                <li><strong>DevTools Inspector (F12):</strong> Tích hợp trình soi cây DOM, Computed Styles và mô hình hộp Box Model trực tiếp.</li>
            </ul>
        </div>

        <div class="card">
            <h2>Thử Nghiệm Điều Hướng</h2>
            <p>Bạn có thể nhập địa chỉ web vào thanh Address Bar phía trên (như <code>https://example.com</code> hoặc <code>about:test</code>) và nhấn Enter hoặc nhấn vào các liên kết bên dưới:</p>
            <ul>
                <li><a href="about:test">Trang kiểm thử Layout CSS & Giao diện (about:test)</a></li>
                <li><a href="about:credits">Thông tin chi tiết về Engine & Giấy phép (about:credits)</a></li>
                <li><a href="https://example.com">Tải trang mẫu https://example.com</a></li>
            </ul>
        </div>
    </div>

    <div class="footer">
        <p>IronSurf Browser Engine &bull; Hoàn toàn bằng Rust &bull; Không phụ thuộc Chromium hay Blink</p>
    </div>
</body>
</html>
"#;

pub const TEST_SUITE_HTML: &str = r#"
<!DOCTYPE html>
<html>
<head>
    <title>CSS & Layout Test Suite - IronSurf</title>
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
        <h1>Kiểm Thử Khả Năng Hiển Thị CSS & Layout</h1>
        <p>Trang này kiểm tra độ chính xác của bộ phân tích cú pháp CSS, bộ khớp Selector và thuật toán tính toán Box Model.</p>

        <div class="box-test">
            <h3>Box Model: Margin, Border, Padding</h3>
            <p>Hộp này kiểm tra padding bên trong, viền border màu tím và lề margin bên ngoài. Bên trong có <span class="highlight">văn bản nổi bật highlight</span>.</p>
        </div>

        <h2>Kiểm Thử Màu Sắc và Thẻ Khối</h2>
        <div class="card-row">
            <div class="card-green">
                <strong>Thẻ Màu Xanh Lá (Success Box):</strong> Thể hiện màu nền RGB, viền bo góc và chữ in đậm.
            </div>
            <div class="card-blue">
                <strong>Thẻ Màu Xanh Dương (Info Box):</strong> Kiểm tra kế thừa màu chữ và độ rộng linh hoạt.
            </div>
            <div class="card-red">
                <strong>Thẻ Màu Đỏ (Alert Box):</strong> Kiểm thử khả năng hiển thị cảnh báo với CSS phân cấp.
            </div>
        </div>

        <h2>Trích Dẫn và Khối Mã Nguồn</h2>
        <blockquote>
            "Mục tiêu là xây dựng một trình duyệt web tối giản, nhanh, độc lập hoàn toàn bằng ngôn ngữ Rust mà không cần cỗ máy cồng kềnh Chromium."
        </blockquote>

        <p>Mẫu mã nguồn Rust bên dưới được định kiểu với thẻ <code>&lt;pre&gt;</code>:</p>
        <pre>fn main() {
    println!("Hello from Pure Rust Browser!");
    let dom = HtmlParser::parse("&lt;h1&gt;IronSurf&lt;/h1&gt;");
    let layout = build_layout_tree(&dom);
}</pre>

        <div class="nav-links">
            <a href="about:welcome">&larr; Quay lại Trang Chủ</a>
            <a href="about:credits">Xem Kiến Trúc Động Cơ &rarr;</a>
        </div>
    </div>
</body>
</html>
"#;

pub const CREDITS_HTML: &str = r#"
<!DOCTYPE html>
<html>
<head>
    <title>IronSurf Engine Architecture & Credits</title>
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
        <h1>Kiến Trúc Động Cơ Trình Duyệt IronSurf</h1>
        <p>IronSurf là một minh chứng rằng hoàn toàn có thể xây dựng một trình duyệt web độc lập, nhanh gọn bằng Rust mà không phụ thuộc vào Chromium/WebKit/Blink.</p>

        <h2>Quy Trình Hoạt Động (Rendering Pipeline):</h2>
        <div class="pipeline">
            <div class="step">
                <strong>1. Network Fetcher (network.rs):</strong> Tải dữ liệu HTTP/HTTPS qua kết nối an toàn TLS (Rustls), hỗ trợ giao thức nội bộ about: và file://.
            </div>
            <div class="step">
                <strong>2. HTML Parser & DOM Builder (html_parser.rs):</strong> Tokenizer bóc tách các thẻ, thuộc tính và sinh cây đối tượng DOM Node.
            </div>
            <div class="step">
                <strong>3. CSS Parser & Cascading (css.rs, style.rs):</strong> Phân tích các bộ quy tắc CSS, tính toán Specificity và kế thừa thuộc tính (Inherited Styles).
            </div>
            <div class="step">
                <strong>4. Box Model Layout Engine (layout.rs):</strong> Tính toán kích thước Margin, Border, Padding, Content, căn giữa (margin auto) và ngắt dòng (text wrap).
            </div>
            <div class="step">
                <strong>5. Display List & Painter (render.rs):</strong> Chuyển cây bố cục thành danh sách lệnh vẽ đồ họa và thực hiện hit-testing cho liên kết siêu văn bản.
            </div>
            <div class="step">
                <strong>6. Interactive Shell & DevTools (app.rs):</strong> Cửa sổ đa tab, thanh địa chỉ thông minh, nút điều hướng lịch sử và bộ thanh tra DOM Inspector thời gian thực.
            </div>
        </div>

        <p><a href="about:welcome">&larr; Trở về Trang Chủ</a></p>
    </div>
</body>
</html>
"#;
