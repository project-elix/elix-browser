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
    // A scheme can only appear at the start, even when a search contains a full URL.
    let scheme = input.split_once(':').filter(|(name, _)| {
        name.starts_with(|ch: char| ch.is_ascii_alphabetic())
            && name
                .bytes()
                .all(|ch| ch.is_ascii_alphanumeric() || b"+-.".contains(&ch))
    });
    let search_operator = scheme.is_some_and(|(name, _)| {
        matches!(
            name.to_ascii_lowercase().as_str(),
            "site" | "filetype" | "intitle" | "inurl" | "intext" | "before" | "after"
        )
    });
    let authority = input.split(['/', '?', '#']).next().unwrap_or(input);
    let host_candidate = !input.contains(char::is_whitespace)
        && (authority.contains('.')
            || authority.starts_with("localhost")
            || authority.starts_with('['));
    let candidate = if scheme.is_some_and(|(_, rest)| rest.starts_with("//")) {
        input.to_owned()
    } else if host_candidate && !search_operator {
        format!("https://{input}")
    } else {
        if let Some((name, rest)) = scheme.filter(|_| !search_operator) {
            // A word followed by ": " introduces ordinary prose. Keep reserved URL
            // schemes out of searches even when a space follows their colon.
            if !rest.starts_with(char::is_whitespace)
                || matches!(
                    name.to_ascii_lowercase().as_str(),
                    "http" | "https" | "elix" | "tauri" | "file" | "data" | "blob"
                        | "javascript" | "vbscript" | "about" | "mailto"
                )
            {
                return Err("Denne protokollen er ikke tillatt".into());
            }
        }
        if input.split_whitespace().any(contains_url_credentials) {
            return Err("Bruk en HTTP- eller HTTPS-adresse uten innloggingsdata".into());
        }
        let mut url = Url::parse("https://www.google.com/search").unwrap();
        url.query_pairs_mut().append_pair("q", input);
        return Ok(url.to_string());
    };
    let url = Url::parse(&candidate).map_err(|_| "Ugyldig adresse")?;
    if !address_allowed(&url) {
        return Err("Bruk en HTTP- eller HTTPS-adresse uten innloggingsdata".into());
    }
    Ok(url.to_string())
}
// Search text can contain a pasted URL or a bare user:password@host.
fn contains_url_credentials(word: &str) -> bool {
    if !word.contains('@') {
        return false;
    }
    let has_userinfo = |url: Url| !url.username().is_empty() || url.password().is_some();
    if Url::parse(word).is_ok_and(has_userinfo) {
        return true;
    }
    // Recognize embedded and malformed URLs even if the whole word won't parse.
    word.split("://").enumerate().any(|(index, part)| {
        let authority = part
            .trim_start_matches(['/', '\\'])
            .split(['/', '\\', '?', '#'])
            .next()
            .unwrap_or_default();
        authority.rsplit_once('@').is_some_and(|(userinfo, _)| {
            !userinfo.is_empty() && (index > 0 || part.starts_with("//") || userinfo.contains(':'))
        })
    })
}
pub fn remote_allowed(url: &Url) -> bool {
    matches!(url.scheme(), "https" | "http") && url.host_str().is_some()
}
fn address_allowed(url: &Url) -> bool {
    remote_allowed(url) && url.username().is_empty() && url.password().is_none()
}

// Restore and handoff load the stored address verbatim, without resolving search text.
pub fn stored_destination_allowed(input: &str) -> bool {
    input.len() <= 8192
        && (INTERNAL_PAGES.contains(&input)
            || Url::parse(input).is_ok_and(|url| address_allowed(&url)))
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
    fn searches_phrases_with_colons() {
        for query in [
            "setning: her",
            "site:github.com",
            "site:github.com Feil: #1",
            "FILETYPE:pdf Rust & nettlesere",
            "filetype:pdf",
            "EKMP setning: her",
            "åpningstid 12:30?",
            "12:30",
            "søk etter https://example.com/?a=1&b=2",
        ] {
            let url = Url::parse(&destination(query).unwrap()).unwrap();
            assert_eq!(url.host_str(), Some("www.google.com"));
            assert_eq!(url.path(), "/search");
            let mut pairs = url.query_pairs();
            let (key, value) = pairs.next().unwrap();
            assert_eq!(key, "q");
            assert_eq!(value, query);
            assert!(pairs.next().is_none());
        }
        assert_eq!(
            destination("example.com:8443/test?q=12:30").unwrap(),
            "https://example.com:8443/test?q=12:30"
        );
    }
    #[test]
    fn stored_destinations_must_already_be_allowed_addresses() {
        for value in [
            "https://example.com/",
            "http://localhost:8000/",
            "elix://start",
        ] {
            assert!(stored_destination_allowed(value), "{value}");
        }
        for value in [
            "setning: her",
            "two words",
            "file:///test",
            "https://user@example.com",
            "elix://unknown",
        ] {
            assert!(!stored_destination_allowed(value), "{value}");
        }
    }
    #[test]
    fn refuses_privileged_schemes_and_credentials() {
        for s in [
            "javascript:alert(1)",
            "JaVaScRiPt: alert(1)",
            "data: text/plain,hello",
            "custom:opaque",
            "søk etter https://user:password@example.com/side",
            "finn (https://user@example.com/side)",
            "søk etter https:/user:password@example.com",
            "søk etter user:password@example.com",
            "SITE:user:password@example.com",
            "site://example.com",
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
