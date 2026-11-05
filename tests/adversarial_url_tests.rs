use rust_browser::network::normalize_url;

#[test]
fn test_valid_domains_and_complex_subdomains() {
    // Standard domains
    assert_eq!(normalize_url("example.com"), "https://example.com");
    assert_eq!(normalize_url("rust-lang.org"), "https://rust-lang.org");
    assert_eq!(normalize_url("crates.io"), "https://crates.io");
    assert_eq!(normalize_url("google.com"), "https://google.com");

    // Complex multi-level subdomains
    assert_eq!(normalize_url("a.b.c.example.com"), "https://a.b.c.example.com");
    assert_eq!(normalize_url("news.ycombinator.com"), "https://news.ycombinator.com");
    assert_eq!(normalize_url("sub-domain.example.co.uk"), "https://sub-domain.example.co.uk");
    assert_eq!(normalize_url("deep.sub.level.co.jp"), "https://deep.sub.level.co.jp");

    // Domains with ports
    assert_eq!(normalize_url("example.com:80"), "https://example.com:80");
    assert_eq!(normalize_url("example.com:8080"), "https://example.com:8080");
    assert_eq!(normalize_url("example.com:65535"), "https://example.com:65535");
    assert_eq!(normalize_url("site.org:443"), "https://site.org:443");

    // Domains with paths
    assert_eq!(normalize_url("example.com/"), "https://example.com/");
    assert_eq!(normalize_url("example.com/path"), "https://example.com/path");
    assert_eq!(normalize_url("example.com/path/to/resource.html"), "https://example.com/path/to/resource.html");

    // Domains with query parameters
    assert_eq!(normalize_url("example.com?q=1"), "https://example.com?q=1");
    assert_eq!(normalize_url("example.com?foo=bar&baz=2"), "https://example.com?foo=bar&baz=2");
    assert_eq!(normalize_url("example.com/search?query=hello+world"), "https://example.com/search?query=hello+world");

    // Domains with fragments
    assert_eq!(normalize_url("example.com#section"), "https://example.com#section");
    assert_eq!(normalize_url("example.com/path#header"), "https://example.com/path#header");

    // Domains with full combination (port + path + query + fragment)
    assert_eq!(
        normalize_url("example.com:8080/path/to?key=value&k2=v2#frag"),
        "https://example.com:8080/path/to?key=value&k2=v2#frag"
    );

    // Case preservation
    assert_eq!(normalize_url("EXAMPLE.COM"), "https://EXAMPLE.COM");
    assert_eq!(normalize_url("Sub.Example.Com:3000/Path"), "https://Sub.Example.Com:3000/Path");

    // Valid hyphens in domains
    assert_eq!(normalize_url("my-domain-test.com"), "https://my-domain-test.com");
    assert_eq!(normalize_url("123-site.org"), "https://123-site.org");
    assert_eq!(normalize_url("site-123.com"), "https://site-123.com");
}

#[test]
fn test_invalid_domains_route_to_search() {
    // Leading or trailing hyphens in domain labels
    assert_eq!(normalize_url("-example.com"), "https://www.google.com/search?q=-example.com");
    assert_eq!(normalize_url("example-.com"), "https://www.google.com/search?q=example-.com");
    assert_eq!(normalize_url("example.-com"), "https://www.google.com/search?q=example.-com");
    assert_eq!(normalize_url("example.com-"), "https://www.google.com/search?q=example.com-");

    // Consecutive dots or leading/trailing dots
    assert_eq!(normalize_url("example..com"), "https://www.google.com/search?q=example..com");
    assert_eq!(normalize_url(".example.com"), "https://www.google.com/search?q=.example.com");
    assert_eq!(normalize_url("example.com."), "https://www.google.com/search?q=example.com.");

    // Invalid port numbers
    assert_eq!(normalize_url("example.com:65536"), "https://www.google.com/search?q=example.com%3A65536");
    assert_eq!(normalize_url("example.com:abc"), "https://www.google.com/search?q=example.com%3Aabc");
    assert_eq!(normalize_url("example.com:"), "https://www.google.com/search?q=example.com%3A");
    assert_eq!(normalize_url("example.com:80.80"), "https://www.google.com/search?q=example.com%3A80.80");

    // TLD length < 2 or non-alphabetic
    assert_eq!(normalize_url("example.c"), "https://www.google.com/search?q=example.c");
    assert_eq!(normalize_url("example.123"), "https://www.google.com/search?q=example.123");
    assert_eq!(normalize_url("example.a1"), "https://www.google.com/search?q=example.a1");
}

#[test]
fn test_non_domain_strings() {
    // Single words
    assert_eq!(normalize_url("rust"), "https://www.google.com/search?q=rust");
    assert_eq!(normalize_url("compiler"), "https://www.google.com/search?q=compiler");
    assert_eq!(normalize_url("search"), "https://www.google.com/search?q=search");
    assert_eq!(normalize_url("browser"), "https://www.google.com/search?q=browser");

    // Multi-word queries
    assert_eq!(
        normalize_url("rust programming language"),
        "https://www.google.com/search?q=rust+programming+language"
    );
    assert_eq!(
        normalize_url("how to build a browser"),
        "https://www.google.com/search?q=how+to+build+a+browser"
    );

    // Version strings
    assert_eq!(normalize_url("1.0"), "https://www.google.com/search?q=1.0");
    assert_eq!(normalize_url("1.2.3"), "https://www.google.com/search?q=1.2.3");
    assert_eq!(normalize_url("v2.3.4"), "https://www.google.com/search?q=v2.3.4");
    assert_eq!(normalize_url("python3.10"), "https://www.google.com/search?q=python3.10");
    assert_eq!(normalize_url("3.14159"), "https://www.google.com/search?q=3.14159");
    assert_eq!(normalize_url("linux-6.1.0"), "https://www.google.com/search?q=linux-6.1.0");

    // Dotfiles
    assert_eq!(normalize_url(".git"), "https://www.google.com/search?q=.git");
    assert_eq!(normalize_url(".gitignore"), "https://www.google.com/search?q=.gitignore");
    assert_eq!(normalize_url(".bashrc"), "https://www.google.com/search?q=.bashrc");
    assert_eq!(normalize_url(".env"), "https://www.google.com/search?q=.env");

    // Punctuation & symbols
    assert_eq!(normalize_url("..."), "https://www.google.com/search?q=...");
    assert_eq!(normalize_url("...."), "https://www.google.com/search?q=....");
    assert_eq!(normalize_url("???"), "https://www.google.com/search?q=%3F%3F%3F");
    assert_eq!(normalize_url("!!!"), "https://www.google.com/search?q=%21%21%21");
    assert_eq!(normalize_url("hello..."), "https://www.google.com/search?q=hello...");
    assert_eq!(normalize_url(".."), "https://www.google.com/search?q=..");
    assert_eq!(normalize_url("."), "https://www.google.com/search?q=.");
    assert_eq!(normalize_url("/"), "https://www.google.com/search?q=%2F");
    assert_eq!(normalize_url("/path"), "https://www.google.com/search?q=%2Fpath");
    assert_eq!(normalize_url("?"), "https://www.google.com/search?q=%3F");
    assert_eq!(normalize_url("#"), "https://www.google.com/search?q=%23");
    assert_eq!(normalize_url(":8080"), "https://www.google.com/search?q=%3A8080");

    // Mathematical expressions
    assert_eq!(normalize_url("2 + 2"), "https://www.google.com/search?q=2+%2B+2");
    assert_eq!(normalize_url("2+2"), "https://www.google.com/search?q=2%2B2");
    assert_eq!(normalize_url("10/2"), "https://www.google.com/search?q=10%2F2");
    assert_eq!(normalize_url("5*5"), "https://www.google.com/search?q=5*5");
    assert_eq!(normalize_url("100 - 50"), "https://www.google.com/search?q=100+-+50");
    assert_eq!(normalize_url("2^8"), "https://www.google.com/search?q=2%5E8");
    assert_eq!(normalize_url("sin(pi/2)"), "https://www.google.com/search?q=sin%28pi%2F2%29");
}

#[test]
fn test_localhost_variants() {
    assert_eq!(normalize_url("localhost"), "http://localhost");
    assert_eq!(normalize_url("localhost:3000"), "http://localhost:3000");
    assert_eq!(normalize_url("localhost:8080"), "http://localhost:8080");
    assert_eq!(normalize_url("localhost/api"), "http://localhost/api");
    assert_eq!(normalize_url("localhost/test/index.html"), "http://localhost/test/index.html");
    assert_eq!(
        normalize_url("localhost:3000/api/v1?token=xyz#admin"),
        "http://localhost:3000/api/v1?token=xyz#admin"
    );
}

#[test]
fn test_ipv4_variants() {
    // Valid IPv4
    assert_eq!(normalize_url("127.0.0.1"), "https://127.0.0.1");
    assert_eq!(normalize_url("127.0.0.1:8000"), "https://127.0.0.1:8000");
    assert_eq!(normalize_url("192.168.1.1"), "https://192.168.1.1");
    assert_eq!(normalize_url("192.168.1.1:8080"), "https://192.168.1.1:8080");
    assert_eq!(normalize_url("10.0.0.1/admin"), "https://10.0.0.1/admin");
    assert_eq!(
        normalize_url("10.0.0.1:8080/status?detail=1#now"),
        "https://10.0.0.1:8080/status?detail=1#now"
    );
    assert_eq!(normalize_url("0.0.0.0"), "https://0.0.0.0");
    assert_eq!(normalize_url("255.255.255.255"), "https://255.255.255.255");

    // Invalid IPv4
    assert_eq!(normalize_url("256.0.0.1"), "https://www.google.com/search?q=256.0.0.1");
    assert_eq!(normalize_url("1.2.3.4.5"), "https://www.google.com/search?q=1.2.3.4.5");
    assert_eq!(normalize_url("999.999.999.999"), "https://www.google.com/search?q=999.999.999.999");
    assert_eq!(normalize_url("127.0.0.1:70000"), "https://www.google.com/search?q=127.0.0.1%3A70000");
    assert_eq!(normalize_url("127.0.0.1:port"), "https://www.google.com/search?q=127.0.0.1%3Aport");
}

#[test]
fn test_encoding_correctness() {
    // Special characters
    assert_eq!(normalize_url("c++"), "https://www.google.com/search?q=c%2B%2B");
    assert_eq!(normalize_url("c++ & rust"), "https://www.google.com/search?q=c%2B%2B+%26+rust");
    assert_eq!(normalize_url("100% pure"), "https://www.google.com/search?q=100%25+pure");
    assert_eq!(normalize_url("#hashtag"), "https://www.google.com/search?q=%23hashtag");
    assert_eq!(normalize_url("query with & and ="), "https://www.google.com/search?q=query+with+%26+and+%3D");

    // Unicode characters
    assert_eq!(normalize_url("Hà Nội"), "https://www.google.com/search?q=H%C3%A0+N%E1%BB%99i");
    assert_eq!(normalize_url("café"), "https://www.google.com/search?q=caf%C3%A9");
    assert_eq!(normalize_url("日本語"), "https://www.google.com/search?q=%E6%97%A5%E6%9C%AC%E8%AA%9E");
    assert_eq!(normalize_url("crab 🦀"), "https://www.google.com/search?q=crab+%F0%9F%A6%80");
}

#[test]
fn test_whitespace_and_scheme_preservation() {
    // Empty & whitespace
    assert_eq!(normalize_url(""), "about:newtab");
    assert_eq!(normalize_url("   "), "about:newtab");
    assert_eq!(normalize_url("\t\n\r"), "about:newtab");

    // Aliases
    assert_eq!(normalize_url("about:welcome"), "about:newtab");
    assert_eq!(normalize_url("about:home"), "about:newtab");
    assert_eq!(normalize_url("about:newtab"), "about:newtab");

    // Schemes preserved
    assert_eq!(normalize_url("about:blank"), "about:blank");
    assert_eq!(normalize_url("about:credits"), "about:credits");
    assert_eq!(normalize_url("about:version"), "about:version");
    assert_eq!(normalize_url("file:///etc/hosts"), "file:///etc/hosts");
    assert_eq!(normalize_url("http://example.com"), "http://example.com");
    assert_eq!(normalize_url("https://example.com"), "https://example.com");
}

#[test]
fn test_stress_and_security_payloads() {
    // XSS / Injection strings should be routed to Google Search safely without panic
    let xss = "<script>alert(1)</script>";
    assert_eq!(
        normalize_url(xss),
        "https://www.google.com/search?q=%3Cscript%3Ealert%281%29%3C%2Fscript%3E"
    );

    let sqli = "SELECT * FROM users WHERE id='1' OR '1'='1'";
    assert_eq!(
        normalize_url(sqli),
        "https://www.google.com/search?q=SELECT+*+FROM+users+WHERE+id%3D%271%27+OR+%271%27%3D%271%27"
    );

    // Large payload (10,000 characters) - ensures no stack overflow or O(n^2) hang
    let large_query = "a".repeat(10000);
    let result = normalize_url(&large_query);
    assert!(result.starts_with("https://www.google.com/search?q=aaaa"));
    assert_eq!(result.len(), "https://www.google.com/search?q=".len() + 10000);

    // Repeated dots
    let dots = ".".repeat(1000);
    assert!(normalize_url(&dots).starts_with("https://www.google.com/search?q=..."));
}

#[test]
fn test_fuzz_and_invariant_stress() {
    let charset: Vec<char> = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789.:/?#-_&+=% \t\n\r!@$^&*()~`{}[]|;\"'<>,\\áéíóú漢字🚀"
        .chars()
        .collect();

    // Deterministic pseudo-random number generator (LCG)
    let mut state: u64 = 0xdeadbeefcafe;
    let mut next_u32 = || -> u32 {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (state >> 32) as u32
    };

    for _ in 0..10_000 {
        let len = (next_u32() % 64) as usize;
        let mut s = String::with_capacity(len);
        for _ in 0..len {
            let idx = (next_u32() as usize) % charset.len();
            s.push(charset[idx]);
        }

        let normalized = normalize_url(&s);

        // Invariant 1: normalize_url must never be empty
        assert!(!normalized.is_empty(), "Normalized URL should never be empty for input: {:?}", s);

        // Invariant 2: Result must always have a recognized valid scheme
        let has_valid_scheme = normalized.starts_with("about:")
            || normalized.starts_with("file://")
            || normalized.starts_with("http://")
            || normalized.starts_with("https://");

        assert!(
            has_valid_scheme,
            "Normalized URL {:?} does not start with a valid scheme for input {:?}",
            normalized, s
        );

        // Invariant 3: If routed to Google Search, query parameter must be present
        if normalized.starts_with("https://www.google.com/search") {
            assert!(
                normalized.starts_with("https://www.google.com/search?q="),
                "Google search URL must have ?q= prefix: {:?}",
                normalized
            );
        }
    }
}

#[test]
fn test_network_manager_search_fetch() {
    use rust_browser::network::{NetworkManager, NetworkMessage};
    use std::time::{Duration, Instant};

    let net = NetworkManager::new();
    // Fetch a raw search query (which should be normalized to Google Search and intercepted)
    net.fetch(42, "rust programming language");

    let start = Instant::now();
    let mut received = None;
    while start.elapsed() < Duration::from_secs(5) {
        if let Some(msg) = net.try_recv() {
            received = Some(msg);
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }

    assert!(received.is_some(), "NetworkManager timed out waiting for search page response");
    match received.unwrap() {
        NetworkMessage::PageLoaded { tab_id, result } => {
            assert_eq!(tab_id, 42);
            let page = result.expect("Search page fetch should succeed");
            assert!(page.url.starts_with("https://www.google.com/search?q="));
            assert!(page.title.contains("Google Search"));
            assert!(page.html.contains("Google Search"));
        }
    }
}
