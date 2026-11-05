# Security Policy

The Aether Rust Browser project takes security and user privacy seriously. As a modern browser engine written from scratch in Rust, our goal is to deliver high-performance, memory-safe, and robust web standards processing while upholding the highest standards of software integrity.

---

## Supported Versions

Only the latest `main` branch and tagged releases are currently supported with security updates.

| Version | Supported          |
| ------- | ------------------ |
| 0.1.x   | :white_check_mark: |
| < 0.1.0 | :x:                |

---

## Reporting a Vulnerability

We strongly encourage coordinated vulnerability disclosure. If you discover a security vulnerability or suspect a potential exploit in Aether Browser, **do not open a public GitHub issue**.

Please report vulnerabilities privately via:
- **Email:** `security@aether-browser.org` (or contact repository maintainers directly)
- **GitHub Security Advisories:** If available, submit a report via GitHub's "Report a vulnerability" tab.

### What to Include in Your Report

To help us triage and resolve the issue quickly, please provide:
1. **Description of the vulnerability:** A detailed summary of the issue, attack vectors, and potential impact.
2. **Steps to Reproduce:** Exact steps, proof-of-concept (PoC) HTML/CSS snippets, or malicious URL payloads triggering the issue.
3. **Environment Details:** Operating System (Linux, macOS, Windows), Rust compiler version (`rustc --version`), and browser build hash.
4. **Estimated Severity:** CVSS score or qualitative assessment (Low, Medium, High, Critical).

### Response SLA & Timeline

- **Acknowledgment:** We aim to acknowledge receipt of all vulnerability reports within **48 hours**.
- **Assessment:** Initial severity triage and confirmation of reproducibility within **5 business days**.
- **Fix & Disclosure:** Fixes will be prepared in a private fork and tested thoroughly. A coordinated public release and security advisory (including CVE assignment where appropriate) will be issued once the fix is deployed.

---

## Threat Model & Engine Architecture Boundaries

### 1. Memory Safety via Rust
Aether leverages Rust's strict ownership model and borrow checker to eliminate broad classes of critical browser vulnerabilities common in C/C++ engines:
- **Buffer overflows and out-of-bounds reads/writes** are eliminated by default.
- **Use-After-Free (UAF)** and **double-free** bugs are prevented at compile time.
- Unsafe Rust usage is restricted and heavily scrutinized across core modules.

### 2. HTML & CSS Parser Isolation
- The HTML tokenizer (`html_parser.rs`) and CSS parser (`css.rs`) are designed with boundary validation and fail-safe recovery.
- Malformed HTML syntax, unclosed tags, recursive nesting, and non-standard CSS declarations recover cleanly without panicking or infinite loops.
- All entities (`&quot;`, `&amp;`, `&#x...;`) undergo strict bounds checking during expansion.

### 3. Network & Transport Security
- Network transactions enforce TLS 1.2 and 1.3 via validated cipher suites.
- Sensitive local schemes (`file://`) and internal control schemes (`about:`) are strictly segregated from public HTTP/HTTPS origins.
- Protocol-relative URLs (`//example.com`) are automatically canonicalized to secure HTTPS transport by default.
- External resource fetching (such as remote CSS stylesheets and image assets) limits execution timeouts to mitigate Denial of Service (DoS) attacks.

### 4. Layout & Rendering Boundary
- Display list construction enforces geometric bounds clamping and coordinate sanity checks, preventing integer overflow during pixel dimension calculations.
- Text fragment tokenization and UTF-8 validation strip invisible bidirectional control sequences and zero-width attack vectors (`\u{200B}`, `\u{FEFF}`, `\u{202E}`) to protect against visual spoofing and homograph attacks.

---

## Security Guidelines for Contributors

1. **Avoid `unsafe` Blocks:** Any introduction of `unsafe` must include a detailed `// SAFETY:` invariant comment and receive explicit maintainer approval.
2. **Fuzzing & Regression Testing:** All parser modifications must pass adversarial stress test suites (`adversarial_url_tests.rs`, `challenger_devtools_stress_tests.rs`, etc.).
3. **Sanitize Inputs:** Never trust raw user inputs or remote server responses without decoding and validation.
4. **Dependency Auditing:** Maintainers regularly run `cargo audit` to identify and update any third-party dependencies with known security advisories.
