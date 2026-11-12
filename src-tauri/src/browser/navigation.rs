pub fn normalize_url(input: &str, search_engine: &str) -> String {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return "https://duckduckgo.com".to_string();
    }

    // Direct schemes
    if trimmed.starts_with("http://")
        || trimmed.starts_with("https://")
        || trimmed.starts_with("file://")
        || trimmed.starts_with("data:")
        || trimmed.starts_with("about:")
    {
        return trimmed.to_string();
    }

    // Localhost
    if trimmed.starts_with("localhost") || trimmed.starts_with("127.0.0.1") || trimmed.starts_with("[::1]") {
        return format!("http://{}", trimmed);
    }

    // Check if looks like domain name
    let has_spaces = trimmed.contains(' ');
    let first_part = trimmed.split('/').next().unwrap_or(trimmed);
    let has_dot = first_part.contains('.');

    if !has_spaces && has_dot && !first_part.ends_with('.') && !first_part.starts_with('.') {
        // Likely a domain (e.g., example.com, sub.example.org/path)
        return format!("https://{}", trimmed);
    }

    // Fallback to Search Engine
    let encoded = urlencoding(trimmed);
    match search_engine.to_lowercase().as_str() {
        "google" => format!("https://www.google.com/search?q={}", encoded),
        "bing" => format!("https://www.bing.com/search?q={}", encoded),
        "startpage" => format!("https://www.startpage.com/do/dsearch?query={}", encoded),
        _ => format!("https://duckduckgo.com/?q={}", encoded), // DuckDuckGo privacy default
    }
}

fn urlencoding(s: &str) -> String {
    let mut result = String::new();
    for b in s.bytes() {
        match b {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                result.push(b as char);
            }
            b' ' => result.push('+'),
            _ => {
                result.push_str(&format!("%{:02X}", b));
            }
        }
    }
    result
}
