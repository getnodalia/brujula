//! Link routing, like Arc's Air Traffic Control: a rule says that the links to a site open in a
//! given space. It applies to the links that arrive from other applications and to the ones kept
//! from Peek or from the small window; what the user types or clicks inside Brújula goes where it
//! always went.

use serde::{Deserialize, Serialize};

use crate::adblock::normalize_host;
use crate::spaces::Spaces;

const MAX_RULES: usize = 50;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rule {
    /// The site, without `www.` (see `adblock::normalize_host`); its subdomains match too.
    pub site: String,
    /// The space where the links open.
    pub space: u32,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Routes {
    items: Vec<Rule>,
}

/// The site a user typed: an address or a bare host.
fn site_of(text: &str) -> Option<String> {
    let t = text.trim();
    if t.starts_with("https://") || t.starts_with("http://") {
        return normalize_host(t);
    }
    if t.contains([':', '/', '?', '#', '@']) {
        return None;
    }
    normalize_host(&format!("https://{t}/"))
}

impl Routes {
    pub fn from_json(text: &str) -> Self {
        let mut r: Self = serde_json::from_str(text).unwrap_or_default();
        let mut seen: Vec<String> = Vec::new();
        r.items.retain_mut(|i| {
            let Some(site) = site_of(&i.site) else {
                return false;
            };
            i.site = site;
            if seen.contains(&i.site) {
                return false;
            }
            seen.push(i.site.clone());
            true
        });
        r.items.truncate(MAX_RULES);
        r
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| "{}".to_owned())
    }

    pub fn items(&self) -> &[Rule] {
        &self.items
    }

    /// Adds a rule, or moves the site's rule to another space. `false` for something that is not a
    /// site, a space that does not exist or too many rules.
    pub fn set(&mut self, site: &str, space: u32, spaces: &Spaces) -> bool {
        let Some(site) = site_of(site) else {
            return false;
        };
        if !spaces.contains(space) {
            return false;
        }
        if let Some(r) = self.items.iter_mut().find(|r| r.site == site) {
            r.space = space;
            return true;
        }
        if self.items.len() >= MAX_RULES {
            return false;
        }
        self.items.push(Rule { site, space });
        true
    }

    pub fn remove(&mut self, site: &str) -> bool {
        let before = self.items.len();
        self.items.retain(|r| r.site != site);
        self.items.len() != before
    }

    /// Rules whose space was deleted go with it.
    pub fn prune(&mut self, spaces: &Spaces) {
        self.items.retain(|r| spaces.contains(r.space));
    }

    /// The space where a link to `url` opens: the rule of its site, or of the closest parent
    /// (`mail.example.com` follows a rule for `example.com`; the more specific rule wins).
    pub fn space_for(&self, url: &str, spaces: &Spaces) -> Option<u32> {
        if !(url.starts_with("https://") || url.starts_with("http://")) {
            return None;
        }
        let host = normalize_host(url)?;
        self.items
            .iter()
            .filter(|r| host == r.site || host.ends_with(&format!(".{}", r.site)))
            .max_by_key(|r| r.site.len())
            .map(|r| r.space)
            .filter(|s| spaces.contains(*s))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spaces() -> (Spaces, u32) {
        let mut s = Spaces::default();
        let work = s.add("Trabajo").unwrap();
        (s, work)
    }

    #[test]
    fn a_rule_sends_a_site_and_its_subdomains_to_a_space() {
        let (spaces, work) = spaces();
        let mut r = Routes::default();
        assert!(r.set("https://www.GitHub.com/x", work, &spaces));
        assert_eq!(r.items()[0].site, "github.com");
        assert_eq!(r.space_for("https://github.com/a/b", &spaces), Some(work));
        assert_eq!(r.space_for("https://gist.github.com/", &spaces), Some(work));
        assert_eq!(r.space_for("https://notgithub.com/", &spaces), None);
        assert_eq!(r.space_for("https://example.org/", &spaces), None);
    }

    #[test]
    fn the_more_specific_rule_wins_and_a_site_has_one_rule() {
        let (spaces, work) = spaces();
        let home = spaces.all()[0].id;
        let mut r = Routes::default();
        assert!(r.set("example.com", work, &spaces));
        assert!(r.set("mail.example.com", home, &spaces));
        assert_eq!(
            r.space_for("https://mail.example.com/", &spaces),
            Some(home)
        );
        assert_eq!(r.space_for("https://example.com/", &spaces), Some(work));
        assert!(r.set("example.com", home, &spaces), "moves the rule");
        assert_eq!(r.items().len(), 2);
        assert_eq!(r.space_for("https://example.com/", &spaces), Some(home));
    }

    #[test]
    fn only_web_addresses_sites_and_existing_spaces_are_taken() {
        let (spaces, work) = spaces();
        let mut r = Routes::default();
        for bad in [
            "",
            "javascript:alert(1)",
            "a b",
            "exa*mple.com",
            "file:///x",
            "x.org/path",
        ] {
            assert!(!r.set(bad, work, &spaces), "{bad}");
        }
        assert!(!r.set("x.org", 999, &spaces));
        assert_eq!(r.space_for("file:///etc/passwd", &spaces), None);
        assert_eq!(r.space_for("javascript:1", &spaces), None);
    }

    #[test]
    fn deleted_spaces_take_their_rules_with_them_and_files_are_read_forgivingly() {
        let (spaces, work) = spaces();
        let mut r = Routes::default();
        r.set("a.org", work, &spaces);
        r.set("b.org", spaces.all()[0].id, &spaces);
        spaces.remove(work);
        assert_eq!(r.space_for("https://a.org/", &spaces), None);
        r.prune(&spaces);
        assert_eq!(r.items().len(), 1);
        let back = Routes::from_json(
            r#"{"items":[{"site":"b.org","space":1},{"site":"b.org","space":2},{"site":"bad host","space":1}]}"#,
        );
        assert_eq!(back.items().len(), 1);
        assert_eq!(Routes::from_json("{no"), Routes::default());
        assert!(r.remove("b.org") && !r.remove("b.org"));
    }
}
