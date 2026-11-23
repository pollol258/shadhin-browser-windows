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
        "brave" => format!("https://search.brave.com/search?q={}", encoded),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_full_schemes() {
        assert_eq!(normalize_url("https://example.com", "duckduckgo"), "https://example.com");
        assert_eq!(normalize_url("http://localhost:3000", "duckduckgo"), "http://localhost:3000");
        assert_eq!(normalize_url("http://127.0.0.1:8080/test", "duckduckgo"), "http://127.0.0.1:8080/test");
        assert_eq!(normalize_url("file:///C:/test.html", "duckduckgo"), "file:///C:/test.html");
        assert_eq!(normalize_url("about:blank", "duckduckgo"), "about:blank");
    }

    #[test]
    fn test_normalize_bare_domains() {
        assert_eq!(normalize_url("example.com", "duckduckgo"), "https://example.com");
        assert_eq!(normalize_url("github.com/pollol258", "duckduckgo"), "https://github.com/pollol258");
        assert_eq!(normalize_url("sub.domain.org/path/file?arg=1", "duckduckgo"), "https://sub.domain.org/path/file?arg=1");
        assert_eq!(normalize_url("localhost:5173", "duckduckgo"), "http://localhost:5173");
    }

    #[test]
    fn test_normalize_search_queries() {
        let query = "rust tauri tutorial";
        assert_eq!(
            normalize_url(query, "duckduckgo"),
            "https://duckduckgo.com/?q=rust+tauri+tutorial"
        );
        assert_eq!(
            normalize_url(query, "google"),
            "https://www.google.com/search?q=rust+tauri+tutorial"
        );
        assert_eq!(
            normalize_url(query, "bing"),
            "https://www.bing.com/search?q=rust+tauri+tutorial"
        );
        assert_eq!(
            normalize_url(query, "brave"),
            "https://search.brave.com/search?q=rust+tauri+tutorial"
        );
    }

    #[test]
    fn test_normalize_empty_input() {
        assert_eq!(normalize_url("", "duckduckgo"), "https://duckduckgo.com");
        assert_eq!(normalize_url("   ", "google"), "https://duckduckgo.com");
    }
}
