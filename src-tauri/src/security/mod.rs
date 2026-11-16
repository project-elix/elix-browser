#[cfg(windows)]
pub mod permissions;
use url::Url;

pub const INTERNAL_PAGES: &[&str] = &[
    "elix://start",
    "elix://developer",
    "elix://settings",
    "elix://profiles",
    "elix://bookmarks",
    "elix://history",
    "elix://downloads",
    "elix://gaming",
];
pub fn destination(input: &str) -> Result<String, String> {
    let input = input.trim();
    if input.len() > 8192 {
        return Err("Adressen er for lang".into());
    }
    if input.is_empty() {
        return Ok("elix://start".into());
    }
    if INTERNAL_PAGES.contains(&input) {
        return Ok(input.into());
    }
    if input.starts_with("elix:") {
        return Err("Ukjent intern side".into());
    }
    // Recognize host:port before rejecting arbitrary protocols.
    let host_candidate = !input.contains(char::is_whitespace)
        && (input.contains('.') || input.starts_with("localhost") || input.starts_with('['));
    let candidate = if input.contains("://") {
        input.to_owned()
    } else if host_candidate {
        format!("https://{input}")
    } else if input.contains(':') {
        return Err("Denne protokollen er ikke tillatt".into());
    } else {
        let mut url = Url::parse("https://www.google.com/search").unwrap();
        url.query_pairs_mut().append_pair("q", input);
        return Ok(url.to_string());
    };
    let url = Url::parse(&candidate).map_err(|_| "Ugyldig adresse")?;
    if !remote_allowed(&url) || !url.username().is_empty() || url.password().is_some() {
        return Err("Bruk en HTTP- eller HTTPS-adresse uten innloggingsdata".into());
    }
    Ok(url.to_string())
}
pub fn remote_allowed(url: &Url) -> bool {
    matches!(url.scheme(), "https" | "http") && url.host_str().is_some()
}
pub fn valid_profile(id: &str) -> bool {
    uuid::Uuid::parse_str(id).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn urls_and_search() {
        assert_eq!(destination("example.com").unwrap(), "https://example.com/");
        assert!(destination("hei lofoten")
            .unwrap()
            .contains("q=hei+lofoten"));
        assert_eq!(destination("elix://developer").unwrap(), "elix://developer");
        assert!(destination("localhost:3000")
            .unwrap()
            .starts_with("https://localhost:3000"));
    }
    #[test]
    fn refuses_privileged_schemes_and_credentials() {
        for s in [
            "javascript:alert(1)",
            "file:///etc/passwd",
            "data:text/html,hi",
            "tauri://localhost",
            "elix://unknown",
            "https://name:pass@example.com",
        ] {
            assert!(destination(s).is_err(), "{s}");
        }
    }
}
