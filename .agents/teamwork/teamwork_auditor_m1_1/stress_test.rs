use rust_browser::network::normalize_url;

fn main() {
    let cases = vec![
        ("google.com", "https://google.com"),
        ("google.com/search?q=test", "https://google.com/search?q=test"),
        ("google.com:443/path#frag", "https://google.com:443/path#frag"),
        ("192.168.1.1", "https://192.168.1.1"),
        ("192.168.1.1:8080/path", "https://192.168.1.1:8080/path"),
        ("256.256.256.256", "https://www.google.com/search?q=256.256.256.256"),
        ("localhost", "http://localhost"),
        ("localhost:3000/api", "http://localhost:3000/api"),
        ("sub.sub2.example.org", "https://sub.sub2.example.org"),
        ("my-service.co.jp", "https://my-service.co.jp"),
        ("-invalid.com", "https://www.google.com/search?q=-invalid.com"),
        ("invalid-.com", "https://www.google.com/search?q=invalid-.com"),
        ("singlelabel", "https://www.google.com/search?q=singlelabel"),
        ("rust 2024 edition", "https://www.google.com/search?q=rust+2024+edition"),
        ("special?query=1&b=2", "https://www.google.com/search?q=special%3Fquery%3D1%26b%3D2"),
        ("what is 2 + 2?", "https://www.google.com/search?q=what+is+2+%2B+2%3F"),
    ];

    let mut failed = 0;
    for (input, expected) in cases {
        let actual = normalize_url(input);
        if actual != expected {
            println!("FAIL: input='{}' -> actual='{}', expected='{}'", input, actual, expected);
            failed += 1;
        } else {
            println!("PASS: input='{}' -> '{}'", input, actual);
        }
    }

    if failed > 0 {
        std::process::exit(1);
    }
}
