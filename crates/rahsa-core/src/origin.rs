//! Origin parsing and matching (PRD §3C rule 1, §11 origin test table).
//!
//! Matching is deliberately strict: `https` only, host compared as the exact
//! ASCII (punycode) string the URL parser produces, ports compared after
//! resolving the default. Subdomain matching must be chosen explicitly.

use serde::{Deserialize, Serialize};
use url::{Host, Url};

use crate::{Error, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum MatchMode {
    #[default]
    Exact,
    Subdomain,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Origin {
    pub scheme: String,
    pub host: String,
    #[serde(default)]
    pub port: Option<u16>,
    #[serde(default, rename = "match")]
    pub match_mode: MatchMode,
}

impl Origin {
    /// Build an origin from something the user typed (a login URL or a bare
    /// origin). Rejects anything that is not `https` with a DNS host.
    pub fn from_user_input(input: &str, match_mode: MatchMode) -> Result<Origin> {
        let url = Url::parse(input.trim()).map_err(|_| Error::Invalid("URL tidak valid".into()))?;
        let o = Origin::from_url(&url).ok_or_else(|| Error::Invalid("URL harus diawali https:// dan memakai nama domain".into()))?;
        Ok(Origin { match_mode, ..o })
    }

    /// The origin of a URL, or `None` if it can never be matched.
    pub fn from_url(url: &Url) -> Option<Origin> {
        if url.scheme() != "https" {
            return None;
        }
        let host = match url.host()? {
            Host::Domain(d) => d.to_string(),
            // IP literals are allowed (intranet servers) but compared literally.
            Host::Ipv4(ip) => ip.to_string(),
            Host::Ipv6(ip) => format!("[{ip}]"),
        };
        if host.is_empty() {
            return None;
        }
        Some(Origin { scheme: "https".into(), host, port: url.port(), match_mode: MatchMode::Exact })
    }

    /// Does the page at `candidate` (a full origin or URL string supplied by
    /// the browser) belong to this registered origin?
    pub fn matches_str(&self, candidate: &str) -> bool {
        match Url::parse(candidate) {
            Ok(u) => self.matches_url(&u),
            Err(_) => false,
        }
    }

    pub fn matches_url(&self, candidate: &Url) -> bool {
        if self.scheme != "https" {
            return false;
        }
        let Some(c) = Origin::from_url(candidate) else { return false };
        if c.port.unwrap_or(443) != self.port.unwrap_or(443) {
            return false;
        }
        match self.match_mode {
            MatchMode::Exact => c.host == self.host,
            MatchMode::Subdomain => {
                // Never treat an IP literal as a parent domain.
                if self.host.parse::<std::net::IpAddr>().is_ok() || self.host.starts_with('[') {
                    return c.host == self.host;
                }
                c.host == self.host
                    || (c.host.len() > self.host.len() + 1
                        && c.host.ends_with(&self.host)
                        && c.host.as_bytes()[c.host.len() - self.host.len() - 1] == b'.')
            }
        }
    }

    /// `https://host[:port]` — what `window.location.origin` would show.
    pub fn serialize(&self) -> String {
        match self.port {
            Some(p) if p != 443 => format!("https://{}:{}", self.host, p),
            _ => format!("https://{}", self.host),
        }
    }

    /// Chrome match pattern(s) used to register the content script for this origin.
    pub fn match_pattern(&self) -> String {
        let host = match self.match_mode {
            MatchMode::Exact => self.host.clone(),
            MatchMode::Subdomain => format!("*.{}", self.host),
        };
        // Chrome match patterns cannot express a port; the extension re-checks
        // the exact origin before doing anything, and so does the app.
        format!("https://{host}/*")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn exact(s: &str) -> Origin {
        Origin::from_user_input(s, MatchMode::Exact).unwrap()
    }

    #[test]
    fn origin_table_from_prd() {
        let o = exact("https://siakad.kampus.ac.id/login");
        assert_eq!(o.host, "siakad.kampus.ac.id");
        assert!(o.matches_str("https://siakad.kampus.ac.id"));
        assert!(o.matches_str("https://siakad.kampus.ac.id/login?next=/"));
        assert!(o.matches_str("https://siakad.kampus.ac.id:443/x"));
        assert!(o.matches_str("https://SIAKAD.Kampus.AC.ID/"));

        let rejected = [
            "https://siakad.kampus.ac.id.evil.com/login", // suffix attack
            "https://evil-siakad.kampus.ac.id/",          // different host
            "https://kampus.ac.id/",                      // parent domain
            "https://sub.siakad.kampus.ac.id/",           // subdomain without opt-in
            "http://siakad.kampus.ac.id/",                // no TLS
            "https://siakad.kampus.ac.id:8443/",          // different port
            "https://siakad.kampus.ac.id./",              // trailing dot
            "https://siakad-kampus.com/",                 // look-alike
            "https://siakad.kаmpus.ac.id/",               // Cyrillic 'а' → punycode
            "https://siakad.kampus.ac.id@evil.com/",      // userinfo trick
            "javascript:alert(1)",
            "file:///C:/siakad.kampus.ac.id",
            "",
            "siakad.kampus.ac.id",
        ];
        for r in rejected {
            assert!(!o.matches_str(r), "should reject {r}");
        }
    }

    #[test]
    fn subdomain_requires_label_boundary() {
        let o = Origin::from_user_input("https://kampus.ac.id", MatchMode::Subdomain).unwrap();
        assert!(o.matches_str("https://sso.kampus.ac.id/"));
        assert!(o.matches_str("https://kampus.ac.id/"));
        assert!(!o.matches_str("https://evilkampus.ac.id/"));
        assert!(!o.matches_str("https://kampus.ac.id.evil.com/"));
        assert!(!o.matches_str("http://sso.kampus.ac.id/"));
    }

    #[test]
    fn rejects_non_https_registration() {
        assert!(Origin::from_user_input("http://siakad.kampus.ac.id", MatchMode::Exact).is_err());
        assert!(Origin::from_user_input("ftp://x.y", MatchMode::Exact).is_err());
        assert!(Origin::from_user_input("not a url", MatchMode::Exact).is_err());
    }

    #[test]
    fn ports_and_serialization() {
        let o = exact("https://intranet.unit.go.id:8443/login");
        assert_eq!(o.port, Some(8443));
        assert_eq!(o.serialize(), "https://intranet.unit.go.id:8443");
        assert!(o.matches_str("https://intranet.unit.go.id:8443/"));
        assert!(!o.matches_str("https://intranet.unit.go.id/"));
        let ip = exact("https://10.0.0.5/login");
        assert!(ip.matches_str("https://10.0.0.5/"));
        assert_eq!(exact("https://a.b/").match_pattern(), "https://a.b/*");
    }

    #[test]
    fn idn_registered_as_punycode() {
        let o = exact("https://bücher.example/");
        assert_eq!(o.host, "xn--bcher-kva.example");
        assert!(o.matches_str("https://xn--bcher-kva.example/"));
        assert!(o.matches_str("https://bücher.example/"));
        assert!(!o.matches_str("https://bucher.example/"));
    }
}
