//! WebView2 reports the main-frame navigation to an attachment as aborted,
//! separately from DownloadStarting. One instance belongs to one native view.
use std::time::{Duration, Instant};

#[derive(Default)]
pub(crate) struct DownloadNavigation {
    id: Option<u64>,
    initial_uri: String,
    latest_uri: String,
    accepted: bool,
    aborted_at: Option<Instant>,
    finished: bool,
}

impl DownloadNavigation {
    pub fn started(&mut self, id: u64, uri: &str) {
        let uri = uri.split('#').next().unwrap_or_default();
        if self.id != Some(id) {
            *self = Self { id: Some(id), initial_uri: uri.into(), ..Self::default() };
        }
        self.latest_uri = uri.into();
    }

    pub fn is_stale(&self, id: u64) -> bool {
        self.id.is_some_and(|current| current != id)
    }

    /// Only an accepted top-level download can explain CONNECTION_ABORTED.
    pub fn completed(&mut self, id: u64, aborted: bool, now: Instant) -> bool {
        if self.id != Some(id) || self.finished { return false; }
        if aborted && !self.accepted {
            self.aborted_at = Some(now);
            return false;
        }
        self.finished = true;
        aborted && self.accepted
    }

    /// An isolated subresource download must never clear another page's error.
    /// The operation can expose either endpoint of the same redirect navigation.
    pub fn accepted(&mut self, uri: &str, now: Instant) -> bool {
        let uri = uri.split('#').next().unwrap_or_default();
        if self.finished || self.id.is_none() || uri.is_empty()
            || (uri != self.initial_uri && uri != self.latest_uri) { return false; }
        self.accepted = true;
        if let Some(aborted) = self.aborted_at {
            self.finished = true;
            // Do not match a separate download of the same URL long after an abort.
            return now.saturating_duration_since(aborted) <= Duration::from_secs(5);
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const PAGE: &str = "https://example.org/file?key=original";
    const CDN: &str = "https://example.net/asset?key=final";

    #[test]
    fn accepts_only_the_matching_download_for_an_aborted_navigation() {
        let now = Instant::now();
        let mut nav = DownloadNavigation::default();
        nav.started(7, PAGE);
        nav.started(7, CDN);
        assert!(!nav.completed(7, true, now));
        assert!(!nav.accepted("https://example.net/asset?key=other", now));
        assert!(nav.accepted(CDN, now));
        assert!(!nav.accepted(CDN, now));
        nav.started(8, PAGE);
        assert!(!nav.accepted(PAGE, now));
        assert!(nav.completed(8, true, now));
    }

    #[test]
    fn preserves_real_failures_stale_completions_and_later_requests() {
        let now = Instant::now();
        let mut nav = DownloadNavigation::default();
        nav.started(1, PAGE);
        assert!(!nav.accepted(PAGE, now));
        assert!(!nav.completed(1, false, now)); // DNS, timeout or rejected HTTP response.
        nav.started(2, CDN);
        assert!(nav.is_stale(1));
        assert!(!nav.completed(1, true, now));
        assert!(!nav.accepted(PAGE, now));
        assert!(!nav.completed(2, true, now));
        assert!(!nav.accepted(CDN, now + Duration::from_secs(10)));
        nav.started(3, PAGE);
        nav.started(4, CDN);
        assert!(!nav.completed(4, true, now));
        assert!(!nav.accepted(PAGE, now));
    }
}
