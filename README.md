# 🦀 IronSurf — Trình Duyệt Web Thuần Rust (100% Pure Rust, Zero Chromium)

**IronSurf** là một trình duyệt web độc lập được xây dựng hoàn toàn từ con số 0 bằng ngôn ngữ lập trình **Rust**, không sử dụng bất kỳ thành phần nào của Chromium, Blink, WebKit, Gecko hay CEF/Electron.

Dự án triển khai trọn vẹn toàn bộ chu trình xử lý (browser pipeline) theo đặc tả web chuẩn: từ kết nối mạng HTTP(S) bảo mật qua Rustls, bộ phân tích cú pháp HTML Tokenizer & cây DOM, bộ phân giải CSS & độ ưu tiên Specificity, mô hình hộp CSS Box Model & ngắt dòng Layout Flow, đến bộ sinh danh sách lệnh vẽ Display List và giao diện người dùng đa thẻ tích hợp công cụ DevTools Inspector.

---

## 🏗 Kiến Trúc Động Cơ (Browser Engine Pipeline)

```
[ URL / Input ] 
       │
       ▼
1. Network Stack (network.rs) ──► Tải mã nguồn HTML qua HTTP/HTTPS (Rustls) hoặc giao thức nội bộ (about:, file://)
       │
       ▼
2. HTML Parser & DOM (html_parser.rs, dom.rs) ──► Tokenizer phân tích thẻ, thuộc tính, thực thể & dựng cây DOM Node
       │
       ▼
3. CSS Parser & Cascade (css.rs, style.rs, default_style.rs) ──► Nạp UA Stylesheet, phân tích Selector, tính Specificity & kế thừa
       │
       ▼
4. Box Model Layout (layout.rs) ──► Tính toán Content, Padding, Border, Margin, căn lề auto và ngắt dòng Inline Flow
       │
       ▼
5. Display List & Painter (render.rs) ──► Chuyển cây bố cục thành lệnh vẽ đồ họa và vùng tương tác (hyperlinks)
       │
       ▼
6. Browser Chrome & DevTools (app.rs, main.rs) ──► Cửa sổ đa thẻ, thanh địa chỉ thông minh, nút điều hướng & DOM Inspector F12
```

---

## 🌟 Các Tính Năng Nổi Bật

### 1. Chu trình kết xuất chuẩn (Complete Rendering Engine)
- **HTML Tokenizer & DOM Builder**: Tự sinh cây đối tượng DOM, xử lý các thẻ tự đóng (void elements như `img`, `meta`, `link`, `br`, `hr`), trích xuất nội dung thẻ thô (`<style>`, `<script>`) và giải mã thực thể HTML (`&amp;`, `&lt;`, `&gt;`, `&copy;`, ...).
- **CSS Engine**:
  - Hỗ trợ các bộ chọn: Universal `*`, Type/Tag (`h1`, `p`, `div`), Class (`.card`), ID (`#main`), Combinators (`div > p`, `nav a`).
  - Tính toán độ ưu tiên Specificity chính xác theo chuẩn W3C: `(ID, Class, Tag)`.
  - Mở rộng cú pháp viết tắt (shorthands): `margin`, `padding`, `border`, `background`.
  - Cơ chế thừa kế kiểu (Style Inheritance) cho kiểu chữ, màu sắc, font-size từ phần tử cha xuống phần tử con.
- **CSS Box Model & Layout Engine**:
  - Tính toán chính xác 4 lớp hộp: Content Box, Padding Box, Border Box, Margin Box.
  - Hỗ trợ căn giữa khối bằng `margin: 0 auto`.
  - Cơ chế Flow Layout thông minh tự động ngắt dòng văn bản (Word Wrapping) và bố trí các phần tử nội dòng (inline links, buttons, inputs).
- **Display List & Hit Testing**:
  - Tạo danh sách lệnh vẽ đồ họa được tối ưu hóa trước khi đưa vào GPU/Painter.
  - Nhận diện con trỏ chuột hover lên các siêu liên kết (`<a>`), đổi con trỏ sang hình bàn tay (`PointingHand`) và kích hoạt điều hướng khi click.

### 2. Giao diện Trình Duyệt Đa Thẻ Hiện Đại (Browser Chrome)
- **Hệ thống Tab đa nhiệm**: Mở tab mới (`+`), chuyển đổi giữa các tab, đóng từng tab (`×`). Mỗi tab độc lập về DOM, lịch sử điều hướng và trạng thái tải.
- **Thanh điều hướng**: Nút Back `⬅`, Forward `➡`, Refresh `🔄`, Home `🏠`.
- **Thanh địa chỉ thông minh (Omnibox)**:
  - Tự động nhận diện URL và tự động bổ sung giao thức an toàn `https://`.
  - Tự động chuyển cụm từ tìm kiếm sang công cụ tìm kiếm nếu không phải là URL.
  - Hiển thị huy hiệu bảo mật: `🔒 HTTPS`, `📄 Internal`, `🔓 HTTP`.
- **Thu phóng trang (Zoom Controls)**: Hỗ trợ phóng to / thu nhỏ từ 50% đến 200%.
- **Thanh trạng thái (Status Bar)**: Hiển thị URL của liên kết đang được rê chuột tới.

### 3. Bộ Công Cụ Lập Trình Tích Hợp (DevTools Inspector F12)
- **DOM Tree Inspector**: Xem cấu trúc phân cấp cây DOM, mở rộng/thu gọn từng nút phần tử và xem chi tiết thuộc tính.
- **Styles Inspector**: Xem danh sách các bộ quy tắc CSS đang áp dụng lên trang.
- **HTML Source**: Xem mã nguồn HTML gốc với phông chữ monospace.
- **Network & Diagnostics**: Đo lường dung lượng tải, thời gian phản hồi và thông tin bảo mật kết nối.

### 4. Các Trang Nội Bộ Có Sẵn (Built-in Pages)
- `about:welcome`: Bảng chào mừng hiện đại với thông số kỹ thuật và các liên kết nhanh.
- `about:test` hoặc `about:acid`: Trang kiểm thử giao diện toàn diện (thẻ card, nút bấm, trích dẫn, mã nguồn, khối màu).
- `about:credits`: Tài liệu chi tiết về kiến trúc động cơ trình duyệt.
- Hỗ trợ tải các trang web thực tế qua Internet (ví dụ: `https://example.com`, `https://news.ycombinator.com`).

---

## 🚀 Hướng Dẫn Cài Đặt & Khởi Chạy

### Yêu cầu môi trường
- Hệ điều hành: Linux, macOS, hoặc Windows.
- Đã cài đặt [Rust](https://rustup.rs/) (phiên bản 1.80+ trở lên).

### 1. Kiểm tra mã nguồn và chạy Unit Tests
```bash
cargo test
```
Tất cả 8 bài kiểm thử chuyên sâu về HTML, CSS, Specificity, Box Model và URL resolution sẽ được thực thi:
```
running 8 tests
test test_css_shorthands_and_colors ... ok
test test_html_parser_basic ... ok
test test_css_parser_rules_and_specificity ... ok
test test_html_style_raw_content ... ok
test test_html_entities_and_void_tags ... ok
test test_layout_box_model_calculation ... ok
test test_url_normalization_and_resolution ... ok
test test_style_cascading_and_inheritance ... ok
```

### 2. Khởi chạy trình duyệt
```bash
cargo run
```
Cửa sổ trình duyệt **IronSurf** sẽ xuất hiện với trang chào mừng `about:welcome`.

---

## 📂 Cấu Trúc Thư Mục Dự Án

```
Rust-browser/
├── Cargo.toml               # Cấu hình phụ thuộc (eframe, reqwest rustls, url, image)
├── README.md                # Tài liệu hướng dẫn sử dụng và giới thiệu kiến trúc
├── src/
│   ├── lib.rs               # Module exports cho thư viện và integration test
│   ├── main.rs              # Điểm khởi chạy ứng dụng GUI (eframe native entrypoint)
│   ├── app.rs               # Giao diện Chrome: Quản lý Tab, Toolbar, Omnibox, DevTools
│   ├── dom.rs               # Cấu trúc dữ liệu DOM: Node, NodeType, ElementData, Attribute map
│   ├── html_parser.rs       # Bộ phân tích cú pháp HTML và xây dựng cây DOM thuần Rust
│   ├── css.rs               # Bộ phân tích cú pháp CSS, bộ chọn Selectors, màu sắc & Shorthands
│   ├── default_style.rs     # User-Agent Stylesheet mặc định cho các thẻ HTML
│   ├── style.rs             # Tính toán độ ưu tiên Specificity, Cascading & kế thừa thuộc tính
│   ├── layout.rs            # Mô hình hộp CSS Box Model & giải thuật dàn trang Flow Layout
│   ├── render.rs            # Bộ sinh danh sách lệnh vẽ Display List & bắt sự kiện tương tác
│   └── network.rs           # Bộ nạp tài nguyên mạng bất đồng bộ (HTTP/HTTPS Rustls, about:, file://)
└── tests/
    └── browser_tests.rs     # Bộ kiểm thử tự động toàn diện cho các thành phần
```

---

## 🛡️ Giấy Phép & Tuyên Bố Bản Quyền
Dự án được xây dựng phục vụ nghiên cứu và phát triển phần mềm hệ thống với ngôn ngữ Rust. 
Hoàn toàn độc lập, không sử dụng mã nguồn Chromium/WebKit/Blink.
