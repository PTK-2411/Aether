# Implementation Analysis & Fix Strategy Report for Milestone M1: Network & Test Suite

## 1. Observation

### 1.1 Toolchain & Baseline Test Verification
- **Command**: `cargo check`
  - **Result**: Exit code 0
  - **Verbatim Output**:
    ```
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.11s
    ```
- **Command**: `cargo test`
  - **Result**: Exit code 0
  - **Verbatim Output**:
    ```
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.18s
         Running unittests src/lib.rs (target/debug/deps/rust_browser-fdfb265d7473f02e)
    running 0 tests
    test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

         Running unittests src/main.rs (target/debug/deps/rust_browser-a4e2acdcd3bc34bf)
    running 0 tests
    test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

         Running tests/browser_tests.rs (target/debug/deps/browser_tests-9848a97b2d53e5b2)
    running 11 tests
    test test_css_shorthands_and_colors ... ok
    test test_html_entities_and_void_tags ... ok
    test test_css_parser_rules_and_specificity ... ok
    test test_html_style_raw_content ... ok
    test test_html_parser_basic ... ok
    test test_layout_box_model_calculation ... ok
    test test_url_normalization_and_resolution ... ok
    test test_style_cascading_and_inheritance ... ok
    test test_metadata_elements_hidden ... ok
    test test_text_word_spacing ... ok
    test test_image_layout_and_display_item ... ok

    test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

       Doc-tests rust_browser
    running 0 tests
    test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
    ```

### 1.2 Current Implementation in `src/network.rs`
- **Imports (`src/network.rs:1-4`)**:
  ```rust
  use std::sync::mpsc::{channel, Receiver, Sender};
  use std::thread;
  use url::Url;
  ```
- **URL Normalization Function (`src/network.rs:48-78`)**:
  ```rust
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

      // Otherwise treat as search query on Aether Search
      let encoded = url::form_urlencoded::byte_serialize(trimmed.as_bytes()).collect::<String>();
      format!("https://search.aether.local/?q={}", encoded)
  }
  ```
- **Search Query Interception (`src/network.rs:155-164`)**:
  ```rust
  // Intercept search queries to return real, rich, clean search results without anti-bot blocks
  if url_str.starts_with("https://search.aether.local")
      || url_str.starts_with("about:search")
      || url_str.starts_with("https://www.google.com/search")
      || url_str.starts_with("https://google.com/search")
      || url_str.starts_with("http://www.google.com/search")
  {
      let query = extract_query_param(url_str, "q").unwrap_or_else(|| "Search".to_string());
      return Ok(generate_aether_search_results(&query, url_str));
  }
  ```
- **Search Page Title & Header (`src/network.rs:367, 436, 455`)**:
  ```rust
  367: <title>{} - Aether Search</title>
  436: <span class="brand-logo">✦ Aether Search</span>
  455: title: format!("{} - Aether Search", query),
  ```

### 1.3 Current Test in `tests/browser_tests.rs`
- **Lines 138-150 (`tests/browser_tests.rs`)**:
  ```rust
  #[test]
  fn test_url_normalization_and_resolution() {
      assert_eq!(normalize_url(""), "about:newtab");
      assert_eq!(normalize_url("about:test"), "about:test");
      assert_eq!(normalize_url("example.com"), "https://example.com");
      assert_eq!(normalize_url("https://rust-lang.org"), "https://rust-lang.org");

      // Relative URLs
      let base = "https://example.com/blog/article.html";
      assert_eq!(resolve_relative_url(base, "about:newtab"), "about:newtab");
      assert_eq!(resolve_relative_url(base, "next.html"), "https://example.com/blog/next.html");
      assert_eq!(resolve_relative_url(base, "/images/logo.png"), "https://example.com/images/logo.png");
  }
  ```

---

## 2. Logic Chain

### 2.1 Trace of User Input to Navigation
1. User enters text in either Omnibox (`src/app.rs:907`) or New Tab center search bar (`src/app.rs:1133`).
2. Input string is passed directly to `BrowserApp::navigate_active_tab(&mut self, target_url: &str)` (`src/app.rs:206`).
3. `navigate_active_tab` calls `normalize_url(target_url)` (`src/app.rs:207`).
4. The normalized URL is assigned to `tab.url`, `self.address_input`, and stored in `tab.history`.
5. `self.network.fetch(tab_id, &normalized)` is called to retrieve and render the page.
6. Therefore, `normalize_url` in `src/network.rs` is the authoritative router for all user navigation and search queries.

### 2.2 Shortcomings of Existing Domain Detection Heuristic
The current heuristic `trimmed.contains('.') && !trimmed.contains(' ')` (`src/network.rs:71`) exhibits specific classification flaws:
- **False Positives (misclassified as domains when they are search queries)**:
  - Version numbers / decimal numbers: `"1.0"`, `"1.2.3"`, `"python3.10"`, `"linux-6.1"` contain a dot and no space, causing them to be incorrectly converted to `https://1.0`, `https://1.2.3`, etc.
  - Punctuation / ellipses: `"hello..."`, `"..."`, `"test."` contain dots and no spaces, incorrectly converted to `https://hello...`.
  - Dotfiles: `".bashrc"` contains a dot and no space, converted to `https://.bashrc`.
- **False Negatives (misclassified as search queries when they are URLs)**:
  - Localhost: `"localhost"`, `"localhost:8080"`, `"localhost/dashboard"` contain no dots, incorrectly falling through to search queries.
- **Search Destination**:
  - Search queries are formatted as `https://search.aether.local/?q={}` instead of Google Search `https://www.google.com/search?q={}`.

### 2.3 Domain & URL Classification Logic Design
To conform strictly with `ORIGINAL_REQUEST.md` (R1) and `PROJECT.md` (§ Interface Contracts):
1. **Empty Strings & Special Aliases**:
   - `""` -> `"about:newtab"`
   - `"about:welcome"`, `"about:newtab"`, `"about:home"` -> `"about:newtab"`
2. **Explicit Schemes**:
   - `about:`, `file://`, `http://`, `https://` -> preserved verbatim.
3. **Localhost Handling**:
   - Inputs matching `"localhost"`, or starting with `"localhost:"` or `"localhost/"` -> format with `http://` (e.g. `http://localhost:8080`).
4. **Direct Domain / IPv4 Navigation (`is_valid_domain_or_ip`)**:
   - Must contain no whitespace: `!trimmed.chars().any(|c| c.is_whitespace())`.
   - Strip path, query, fragment: extract `authority` segment prior to `/`, `?`, or `#`.
   - Extract port suffix: if `authority` contains `:`, verify the suffix parses as a valid `u16` port (1-65535); extract the `host` prefix.
   - **IPv4 validation**: if `host.parse::<std::net::Ipv4Addr>().is_ok()` -> valid host (e.g., `127.0.0.1`, `192.168.1.1:8080`).
   - **Named Domain validation**:
     - Must contain at least one dot: `host.contains('.')`.
     - Must not start with `.` or end with `.`.
     - Split into labels by `.`: `labels.len() >= 2`.
     - For each label: non-empty, contains only ASCII alphanumeric characters and hyphens, and neither starts nor ends with a hyphen (RFC 1035 / RFC 1123).
     - For the TLD (the final label): `tld.len() >= 2` AND all characters must be ASCII alphabetic (`tld.chars().all(|c| c.is_ascii_alphabetic())`).
   - If valid -> prepend `https://` (e.g. `https://example.com`, `https://rust-lang.org/learn`).
5. **Search Query Routing**:
   - If not a valid domain or URL -> format to `https://www.google.com/search?q={encoded}`.
   - Query encoding via `url::form_urlencoded::byte_serialize(trimmed.as_bytes()).collect::<String>()` (already imported and used across `src/network.rs`).

---

## 3. Caveats

1. **Search Interception vs Live Google Requests**:
   `fetch_url_content` in `src/network.rs:158` intentionally intercepts `https://www.google.com/search` to serve Wikipedia/encyclopedia search cards and avoid Google's anti-bot/CAPTCHA blocking. The displayed URL in the Omnibox and tab history correctly remains `https://www.google.com/search?q=...`.
2. **Search Suggestion Dropdown in `src/app.rs`**:
   While `normalize_url` handles all text submitted via the Enter key in the Omnibox and center search bar, `src/app.rs:418` has a hardcoded suggestion click handler (`navigate_to` -> `https://search.aether.local/?q={}`). When the UI implementer works on `src/app.rs`, line 418 should be updated to `self.navigate_active_tab(&query);` so suggestions delegate directly through `normalize_url`.
3. **No External Crates Needed**:
   Domain detection and IPv4 parsing are achieved exclusively with `std::net::Ipv4Addr` and standard string slices, respecting the project constraint: *"The team must NOT: Use pre-built libraries or frameworks for core functionality"*.

---

## 4. Conclusion & Proposed Implementation Specification

### 4.1 Proposed Changes to `src/network.rs`

#### Change 1: Add `Ipv4Addr` to imports
**File**: `src/network.rs`
**Line**: 1
```diff
--- a/src/network.rs
+++ b/src/network.rs
@@ -1,3 +1,4 @@
+use std::net::Ipv4Addr;
 use std::sync::mpsc::{channel, Receiver, Sender};
 use std::thread;
 use url::Url;
```

#### Change 2: Add `is_valid_domain_or_ip` helper and update `normalize_url`
**File**: `src/network.rs`
**Lines**: 48-78

```rust
// Proposed Implementation for lines 48-120:

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
```

#### Change 3: Update Search Result Branding in `src/network.rs`
**File**: `src/network.rs`
**Lines**: 367, 436, 455
Update search page titles and branding from `"Aether Search"` to `"Google Search"`:
- Line 367: `<title>{} - Google Search</title>`
- Line 436: `<span class="brand-logo">Google Search</span>`
- Line 455: `title: format!("{} - Google Search", query),`

---

### 4.2 Proposed Changes to `tests/browser_tests.rs`

Expand `tests/browser_tests.rs:138-150` with thorough coverage for Google search routing, domain formats, IPv4, localhost, and edge cases:

```rust
// Proposed replacement for lines 138-150:

#[test]
fn test_url_normalization_and_resolution() {
    // Empty & about: aliases
    assert_eq!(normalize_url(""), "about:newtab");
    assert_eq!(normalize_url("   "), "about:newtab");
    assert_eq!(normalize_url("about:welcome"), "about:newtab");
    assert_eq!(normalize_url("about:home"), "about:newtab");
    assert_eq!(normalize_url("about:test"), "about:test");

    // Standard schemes preserved verbatim
    assert_eq!(normalize_url("file:///etc/hosts"), "file:///etc/hosts");
    assert_eq!(normalize_url("http://example.com"), "http://example.com");
    assert_eq!(normalize_url("https://rust-lang.org"), "https://rust-lang.org");

    // Localhost routing (http)
    assert_eq!(normalize_url("localhost"), "http://localhost");
    assert_eq!(normalize_url("localhost:8080"), "http://localhost:8080");
    assert_eq!(normalize_url("localhost/dashboard"), "http://localhost/dashboard");

    // Valid domains (https)
    assert_eq!(normalize_url("example.com"), "https://example.com");
    assert_eq!(normalize_url("news.ycombinator.com"), "https://news.ycombinator.com");
    assert_eq!(normalize_url("rust-lang.org/learn"), "https://rust-lang.org/learn");
    assert_eq!(normalize_url("crates.io:443/crates/url"), "https://crates.io:443/crates/url");
    assert_eq!(normalize_url("sub-domain.example.co.uk"), "https://sub-domain.example.co.uk");

    // IPv4 addresses (https)
    assert_eq!(normalize_url("127.0.0.1"), "https://127.0.0.1");
    assert_eq!(normalize_url("192.168.1.1:8080"), "https://192.168.1.1:8080");
    assert_eq!(normalize_url("10.0.0.1/admin"), "https://10.0.0.1/admin");

    // Google Search routing for queries, words, phrases, and special characters
    assert_eq!(normalize_url("rust"), "https://www.google.com/search?q=rust");
    assert_eq!(
        normalize_url("rust programming language"),
        "https://www.google.com/search?q=rust+programming+language"
    );
    assert_eq!(
        normalize_url("c++ & rust"),
        "https://www.google.com/search?q=c%2B%2B+%26+rust"
    );

    // Queries with dots/numbers that are NOT valid domains (routed to Google Search)
    assert_eq!(normalize_url("1.0"), "https://www.google.com/search?q=1.0");
    assert_eq!(normalize_url("1.2.3"), "https://www.google.com/search?q=1.2.3");
    assert_eq!(normalize_url("v1.2"), "https://www.google.com/search?q=v1.2");
    assert_eq!(normalize_url("python3.10"), "https://www.google.com/search?q=python3.10");
    assert_eq!(normalize_url("hello..."), "https://www.google.com/search?q=hello...");
    assert_eq!(normalize_url("..."), "https://www.google.com/search?q=...");
    assert_eq!(normalize_url(".bashrc"), "https://www.google.com/search?q=.bashrc");

    // Relative URLs resolution
    let base = "https://example.com/blog/article.html";
    assert_eq!(resolve_relative_url(base, "about:newtab"), "about:newtab");
    assert_eq!(resolve_relative_url(base, "next.html"), "https://example.com/blog/next.html");
    assert_eq!(resolve_relative_url(base, "/images/logo.png"), "https://example.com/images/logo.png");
}
```

---

## 5. Verification Method

To independently verify the implementation after code changes:

1. **Build & Typecheck**:
   ```bash
   cargo check
   ```
   *Expected*: Exit code 0, 0 errors, 0 warnings.
2. **Execute Full Test Suite**:
   ```bash
   cargo test
   ```
   *Expected*: All unit and integration tests (including expanded `test_url_normalization_and_resolution`) exit 0 with all tests passing.
3. **Execute Targeted URL Normalization Test**:
   ```bash
   cargo test --test browser_tests test_url_normalization_and_resolution -- --nocapture
   ```
   *Expected*: Passes showing all domain and search assertions succeed.
4. **Invalidation Conditions**:
   - If `normalize_url("rust")` produces any URL other than `https://www.google.com/search?q=rust`.
   - If `normalize_url("1.0")` produces `https://1.0` instead of a Google search.
   - If `normalize_url("localhost:8080")` produces a Google search URL instead of `http://localhost:8080`.
   - If `cargo test` fails any assertions.
