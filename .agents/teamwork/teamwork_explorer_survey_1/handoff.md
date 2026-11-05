# Search Navigation Architecture & Implementation Survey

## 1. Observation

### 1.1 Toolchain & Build Verification Commands & Results
- **Command**: `cargo check`
  - **Result**: Exit code 0.
  - **Output**:
    ```
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.10s
    ```
- **Command**: `cargo test`
  - **Result**: Exit code 0. All 11 unit/integration tests passed in 0.00s.
  - **Output**:
    ```
    running 11 tests
    test test_css_parser_rules_and_specificity ... ok
    test test_html_entities_and_void_tags ... ok
    test test_css_shorthands_and_colors ... ok
    test test_html_style_raw_content ... ok
    test test_html_parser_basic ... ok
    test test_layout_box_model_calculation ... ok
    test test_style_cascading_and_inheritance ... ok
    test test_url_normalization_and_resolution ... ok
    test test_image_layout_and_display_item ... ok
    test test_metadata_elements_hidden ... ok
    test test_text_word_spacing ... ok

    test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
    ```

### 1.2 Dependencies (`Cargo.toml`)
- `Cargo.toml` lines 6-12:
  ```toml
  [dependencies]
  eframe = "0.31"
  reqwest = { version = "0.12", default-features = false, features = ["rustls-tls", "blocking"] }
  url = "2.5"
  image = { version = "0.25", default-features = false, features = ["png", "jpeg", "webp"] }
  serde_json = "1.0"
  ```
  `url = "2.5"` is already present as an approved project dependency providing `url::form_urlencoded::byte_serialize` and `url::Url`.

### 1.3 Omnibox Input Handling (`src/app.rs`)
- Text buffer: `self.address_input: String` (`src/app.rs:101`).
- Rendering & Submit logic (`src/app.rs:887-908`):
  ```rust
  // URL TextEdit
  let available_edit_w = child_ui.available_width() - 85.0;
  let text_edit = child_ui.add_sized(
      Vec2::new(available_edit_w.max(50.0), 24.0),
      egui::TextEdit::singleline(&mut self.address_input)
          .hint_text("Search with Aether or type a URL")
          .frame(false)
          .text_color(Color32::from_rgb(232, 234, 237)),
  );

  if text_edit.changed() {
      self.show_omnibox_suggestions = true;
      let input = self.address_input.clone();
      self.trigger_suggestions(&input);
  }

  if text_edit.lost_focus() && ctx.input(|i| i.key_pressed(Key::Enter)) {
      self.show_omnibox_suggestions = false;
      self.search_suggestions.clear();
      let target = self.address_input.clone();
      self.navigate_active_tab(&target);
  }
  ```

### 1.4 New Tab Center Search Bar Input Handling (`src/app.rs`)
- Text buffer: `self.center_search_input: String` (`src/app.rs:102`).
- Rendering & Submit logic (`src/app.rs:1111-1135`):
  ```rust
  // Text Edit: "Search with Aether or type a URL"
  let input_width = inner_ui.available_width() - 175.0;
  let center_edit = inner_ui.add_sized(
      Vec2::new(input_width.max(100.0), 30.0),
      egui::TextEdit::singleline(&mut self.center_search_input)
          .hint_text("Search with Aether or type a URL")
          .frame(false)
          .text_color(Color32::WHITE)
          .font(FontId::proportional(15.0)),
  );

  if center_edit.changed() {
      self.show_center_suggestions = true;
      let input = self.center_search_input.clone();
      self.trigger_suggestions(&input);
  }

  if center_edit.lost_focus() && ctx.input(|i| i.key_pressed(Key::Enter)) {
      self.show_center_suggestions = false;
      self.search_suggestions.clear();
      let query = self.center_search_input.clone();
      if !query.trim().is_empty() {
          self.navigate_active_tab(&query);
      }
  }
  ```

### 1.5 Autocomplete Dropdown Selection Handling (`src/app.rs`)
- Rendering & Click logic (`src/app.rs:407-420`):
  ```rust
  if row_resp.clicked() {
      navigate_to = Some(suggestion.clone());
  }
  ...
  if let Some(query) = navigate_to {
      self.search_suggestions.clear();
      self.show_omnibox_suggestions = false;
      self.show_center_suggestions = false;
      self.navigate_active_tab(&format!("https://search.aether.local/?q={}", query));
  }
  ```
  Note: Clicking an autocomplete item currently hardcodes `https://search.aether.local/?q={}` at line 418.

### 1.6 Active Tab Navigation Dispatcher (`src/app.rs`)
- Implementation (`src/app.rs:206-233`):
  ```rust
  pub fn navigate_active_tab(&mut self, target_url: &str) {
      let normalized = normalize_url(target_url);
      if let Some(tab) = self.tabs.get_mut(self.active_tab_idx) {
          let is_newtab = normalized == "about:newtab" || normalized == "about:welcome" || normalized == "about:home";
          tab.is_loading = !is_newtab;
          tab.url = normalized.clone();
          tab.scroll_y = 0.0;

          if is_newtab {
              tab.title = "New Tab".to_string();
              self.address_input = String::new();
          } else {
              tab.title = "Loading...".to_string();
              self.address_input = normalized.clone();
          }

          if tab.history_index + 1 < tab.history.len() {
              tab.history.truncate(tab.history_index + 1);
          }
          tab.history.push(normalized.clone());
          tab.history_index = tab.history.len() - 1;

          if !is_newtab {
              let tab_id = tab.id;
              self.network.fetch(tab_id, &normalized);
          }
      }
  }
  ```

### 1.7 Current URL Normalization & Search Routing Function (`src/network.rs`)
- Implementation (`src/network.rs:48-78`):
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

### 1.8 Search Query Interception in Network Fetch (`src/network.rs`)
- Handling in `fetch_url_content` (`src/network.rs:155-164`):
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
  `fetch_url_content` already recognizes `https://www.google.com/search` and intercepts it via `generate_aether_search_results`, which populates `PageResponse` with `url: url_str.to_string()`.

### 1.9 Search Background Styling in Canvas (`src/app.rs`)
- Canvas styling (`src/app.rs:1400-1411`):
  ```rust
  // Webpage canvas background
  let canvas_bg = if current_tab.url.contains("search.aether.local")
      || current_tab.url.contains("google.com/search")
      || current_tab.url.contains("about:ai")
  {
      Color32::from_rgb(32, 33, 36)
  ...
  ```
  Canvas styling already supports `google.com/search`.

---

## 2. Logic Chain

### 2.1 Trace of Search Navigation Authority
1. User enters text in either the Omnibox (`src/app.rs:907`) or the New Tab search bar (`src/app.rs:1133`) and presses Enter.
2. In both locations, the raw string is handed unmodified to `BrowserApp::navigate_active_tab(&mut self, target_url: &str)` (`src/app.rs:206`).
3. `navigate_active_tab` calls `normalize_url(target_url)` at line 207 to obtain `normalized`.
4. The active tab's URL (`tab.url`), tab history (`tab.history`), and the Omnibox display (`self.address_input`) are all set to `normalized`.
5. `self.network.fetch(tab_id, &normalized)` is called to retrieve page content.
6. `NetworkManager::fetch` also calls `normalize_url(raw_url)` at `src/network.rs:36` before dispatching.
7. Therefore, **`pub fn normalize_url(input: &str) -> String` in `src/network.rs:48` is the authoritative single source of truth** for URL resolution and search query routing across the application.

### 2.2 Distinguishing URLs/Domains vs Arbitrary Search Queries
1. **Explicit URL Schemes**:
   - `about:` (e.g. `about:newtab`, `about:test`, `about:credits`, `about:ai`, `about:blank`)
   - `file://` (local filesystem)
   - `http://` and `https://` (standard web URLs)
   These are explicitly checked via `starts_with` and immediately preserved without modification.
2. **Domain vs Search Query Evaluation**:
   - The current heuristic (`src/network.rs:71`):
     ```rust
     if trimmed.contains('.') && !trimmed.contains(' ') {
         return format!("https://{}", trimmed);
     }
     ```
   - **Correct behaviors of current heuristic**:
     - Standard domains like `example.com`, `news.ycombinator.com`, `rust-lang.org/learn`, `github.com` contain `.` and no spaces -> correctly converted to `https://...`.
     - Multi-word queries like `rust tutorial`, `aether browser speed` contain spaces -> correctly bypass domain check and become search queries.
     - Single-word queries like `rust`, `google`, `ferris`, `weather` contain no `.` -> correctly bypass domain check and become search queries.
   - **Identified Edge Cases & Gaps**:
     - *Localhost*: `localhost` or `localhost:8080` has no `.` and no space. Currently, typing `localhost` or `localhost:8080` wrongly becomes a search query instead of `http://localhost:8080`.
     - *Numeric / Version strings*: Typing `1.0` or `v1.2` or `1.80` without spaces has a `.` and no space, causing it to be treated as `https://1.0`.
     - *Punctuation / Trailing dots*: Query `hello...` has a `.` and no space, currently treated as `https://hello...`.
3. **Accurate & Robust Domain Detection Design**:
   A robust, pure-Rust domain detector should verify standard domain structure without external libraries:
   - Extract the host slice (the segment before any `/`, `?`, `#`, or `:`).
   - Reject if host contains spaces or control characters.
   - Recognize `localhost` explicitly (`trimmed == "localhost" || trimmed.starts_with("localhost:") || trimmed.starts_with("localhost/")`).
   - Check if host parses as a valid IPv4 address (`std::net::Ipv4Addr::from_str(host).is_ok()`).
   - For named domains:
     - Must not start with `.` or end with `.`.
     - Split host by `.`. Must have at least 2 non-empty labels.
     - The top-level domain (TLD, last label) must be at least 2 characters long and consist solely of ASCII letters (`tld.len() >= 2 && tld.chars().all(|c| c.is_ascii_alphabetic())`).
     - Domain labels must only contain ASCII alphanumeric characters and hyphens, and cannot start or end with a hyphen.
   - If the input meets this standard domain format, prepend `https://` (or `http://` for localhost).
   - If it does not, treat it as a search query.

### 2.3 Constructing Google Search Navigation & Parameter Encoding
1. **Target URL Structure**:
   `https://www.google.com/search?q=<encoded_query>`
2. **Encoding Compliance & Constraints**:
   - The integrity constraint specifies: "The team must NOT: Use pre-built libraries or frameworks for core functionality".
   - `url = "2.5"` is already in `Cargo.toml:9` and is actively used across `src/network.rs` (lines 76, 259, 281) and `src/app.rs` (line 484):
     ```rust
     let encoded = url::form_urlencoded::byte_serialize(trimmed.as_bytes()).collect::<String>();
     ```
   - Alternatively, a lightweight, 15-line standard percent-encoding function can be implemented in `src/network.rs` with zero external dependencies:
     ```rust
     fn encode_query_param(s: &str) -> String {
         let mut out = String::with_capacity(s.len() * 3 / 2);
         for b in s.as_bytes() {
             match b {
                 b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(*b as char),
                 b' ' => out.push('+'),
                 _ => out.push_str(&format!("%{:02X}", b)),
             }
         }
         out
     }
     ```
   - In `src/network.rs:77`, changing:
     ```rust
     // Before:
     format!("https://search.aether.local/?q={}", encoded)
     // After:
     format!("https://www.google.com/search?q={}", encoded)
     ```
     immediately routes all search queries from both Omnibox and New Tab search bar to Google Search.
3. **Autocomplete Dropdown Consistency**:
   - In `src/app.rs:418`, the suggestion dropdown click handler currently executes:
     `self.navigate_active_tab(&format!("https://search.aether.local/?q={}", query));`
   - It should be aligned with the rest of navigation:
     `self.navigate_active_tab(&query);`
     Passing `&query` directly to `self.navigate_active_tab(&query)` delegates to `normalize_url(&query)`, automatically handling both search queries (routing to `https://www.google.com/search?q=...`) and domain suggestions (like `crates.io` -> `https://crates.io`).

### 2.4 Downstream Impact on Page Rendering
1. When `self.navigate_active_tab("https://www.google.com/search?q=...")` is called:
   - `tab.url` is set to `"https://www.google.com/search?q=..."`.
   - `NetworkManager::fetch` calls `fetch_url_content`.
   - `fetch_url_content` (`src/network.rs:158`) checks `url_str.starts_with("https://www.google.com/search")`.
   - It extracts `q` using `extract_query_param(url_str, "q")` and generates the search results page via `generate_aether_search_results(&query, url_str)`.
   - `PageResponse` returns `url: url_str.to_string()`, keeping the tab URL as `https://www.google.com/search?q=...`.
   - `src/app.rs:1402` already gives `current_tab.url.contains("google.com/search")` the dark search canvas background.

---

## 3. Caveats

1. **Anti-Bot Interception of Google Search**:
   Directly making raw HTTP requests from a custom Rust browser to live `https://www.google.com/search?q=...` endpoints is blocked by Google's anti-bot/CAPTCHA systems and requires complex JavaScript. The existing codebase intentionally intercepts `https://www.google.com/search` in `fetch_url_content` (`src/network.rs:158`) to render clean, readable search results backed by Wikipedia's open API and fallback search cards. The URL displayed in the Omnibox and stored in tab history remains `https://www.google.com/search?q=...`.
2. **Existing Unit Test Scope**:
   Existing tests in `tests/browser_tests.rs:140-143` (`test_url_normalization_and_resolution`) verify `""`, `"about:test"`, `"example.com"`, and `"https://rust-lang.org"`, but do not assert the destination of search queries. Changing search normalization to Google Search will not break any existing tests, and new test assertions for Google search should be added.
3. **Autocomplete Dropdown vs Direct Submission**:
   There are two distinct code locations where search queries are submitted:
   - Pressing Enter in Omnibox or Center Search Bar -> passes to `self.navigate_active_tab(&input)`.
   - Clicking an item in the search autocomplete dropdown (`src/app.rs:418`) -> previously had a hardcoded `https://search.aether.local/?q={query}`. Both must be updated to ensure consistency.

---

## 4. Conclusion

1. **Authoritative Dispatcher**:
   `pub fn normalize_url(input: &str) -> String` located in `src/network.rs:48` is the central source of truth for converting user input into either a direct URL or a search engine query.
2. **Current State**:
   - `normalize_url` routes queries to `https://search.aether.local/?q={encoded}`.
   - Autocomplete dropdown click at `src/app.rs:418` hardcodes `https://search.aether.local/?q={query}`.
   - Valid domains are detected via `trimmed.contains('.') && !trimmed.contains(' ')`.
3. **Required Modifications for Implementation**:
   - **`src/network.rs`**:
     1. In `normalize_url`, route search queries to `format!("https://www.google.com/search?q={}", encoded)`.
     2. Enhance domain detection to validate standard domain formats (e.g. host separation, TLD alphabetic check of length >= 2, IPv4 validation, localhost recognition) to eliminate false positives on version numbers or punctuation without spaces.
   - **`src/app.rs`**:
     1. In `render_search_suggestions_dropdown` line 418, replace hardcoded `https://search.aether.local/?q={}` with `self.navigate_active_tab(&query)`.
     2. Optionally update Omnibox and New Tab search bar hint text from `"Search with Aether or type a URL"` to `"Search with Google or type a URL"` if desired for UI polish.
   - **`tests/browser_tests.rs`**:
     Add test cases to `test_url_normalization_and_resolution` for search queries:
     ```rust
     assert_eq!(normalize_url("rust language"), "https://www.google.com/search?q=rust+language");
     assert_eq!(normalize_url("ferris"), "https://www.google.com/search?q=ferris");
     ```
4. **Current Build & Test Health**:
   - `cargo check`: Pass (0 errors, 0 warnings).
   - `cargo test`: Pass (11/11 tests passing).

---

## 5. Verification Method

To independently verify the survey findings:

1. **Verify Cargo Check**:
   ```bash
   cargo check
   ```
   *Expected*: Compiles without errors.
2. **Verify Cargo Test Suite**:
   ```bash
   cargo test
   ```
   *Expected*: 11 tests in `tests/browser_tests.rs` run and pass.
3. **Inspect URL Normalization Logic**:
   Examine lines 48-78 in `src/network.rs`:
   ```bash
   sed -n '48,78p' src/network.rs
   ```
4. **Inspect Input Handling in `src/app.rs`**:
   - Omnibox Enter handling: lines 903-908.
   - Center search bar Enter handling: lines 1128-1135.
   - Suggestions dropdown click: lines 414-419.
5. **Invalidation Conditions**:
   - If `normalize_url` is removed or its signature changed without updating `navigate_active_tab` or `tests/browser_tests.rs`.
   - If `fetch_url_content` search query interception pattern (`src/network.rs:158`) is altered.
