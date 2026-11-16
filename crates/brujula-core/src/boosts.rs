//! Boosts: a site's look changed by the user, as Arc's boosts. Each one is a dark mode switch,
//! a font, a background and text colour, the elements hidden with Zap and some CSS of the user's
//! own, and applies only to that site and its subdomains.
//!
//! Only style: no script, so a boost cannot read or send what the page holds. The CSS is the
//! user's, written in the preferences, and is injected as a user style sheet (never as HTML). The
//! rest is chosen from fixed lists or checked (`valid_selector`, `valid_color`) before it goes
//! into the sheet, so nothing typed or picked can close a rule and open another.

use serde::{Deserialize, Serialize};

use crate::adblock::normalize_host;

/// Longest CSS of one boost, in characters.
pub const MAX_CSS: usize = 8 * 1024;
const MAX_BOOSTS: usize = 100;
/// How many elements a site can have hidden.
pub const MAX_HIDDEN: usize = 40;
const MAX_SELECTOR: usize = 240;

/// The fonts a boost can ask for: id, name and the `font-family` it puts.
pub const FONTS: &[(&str, &str, &str)] = &[
    ("sans", "Sans del sistema", "system-ui, sans-serif"),
    ("inter", "Inter", "'Inter', system-ui, sans-serif"),
    ("sora", "Sora", "'Sora', system-ui, sans-serif"),
    ("serif", "Con serifa", "Georgia, 'Times New Roman', serif"),
    (
        "mono",
        "Monoespaciada",
        "ui-monospace, 'DejaVu Sans Mono', monospace",
    ),
];

/// A selector a boost may hide: what Zap writes and what the user may type. Letters, digits and
/// the few signs a selector needs; nothing that could end the rule (`{`, `}`, `;`), start a
/// comment (`/*`) or an at-rule (`@`), or break out of the sheet (`<`, `\`, `!`).
pub fn valid_selector(selector: &str) -> bool {
    let s = selector.trim();
    !s.is_empty()
        && s.chars().count() <= MAX_SELECTOR
        && s.chars().all(|c| {
            c.is_alphanumeric()
                || matches!(
                    c,
                    '-' | '_'
                        | '#'
                        | '.'
                        | '>'
                        | '+'
                        | '~'
                        | ':'
                        | '('
                        | ')'
                        | '['
                        | ']'
                        | '='
                        | '"'
                        | '\''
                        | '*'
                        | ' '
                        | '^'
                        | '$'
                        | '|'
                        | '/'
                )
        })
        && !s.contains("/*")
        && !s.contains("*/")
        && s.matches('(').count() == s.matches(')').count()
        && s.matches('[').count() == s.matches(']').count()
        && s.matches('"').count() & 1 == 0
        && s.matches('\'').count() & 1 == 0
}

/// `#rrggbb`, the only colour a boost takes.
pub fn valid_color(color: &str) -> bool {
    color.len() == 7 && color.starts_with('#') && color[1..].chars().all(|c| c.is_ascii_hexdigit())
}

/// Dark mode for pages that have none: the colours are inverted, and what should not be
/// (pictures, video, embedded pages) is inverted back.
const DARK: &str = "html { filter: invert(1) hue-rotate(180deg) !important; background: #fff; }\n\
img, picture, video, canvas, iframe, svg image { filter: invert(1) hue-rotate(180deg) !important; }\n";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Boost {
    /// The site, without `www.` (see `adblock::normalize_host`).
    pub host: String,
    #[serde(default)]
    pub dark: bool,
    #[serde(default)]
    pub css: String,
    /// The elements hidden with Zap: one selector each.
    #[serde(default)]
    pub hidden: Vec<String>,
    /// The id of one of `FONTS`, or empty to leave the site's own.
    #[serde(default)]
    pub font: String,
    /// `#rrggbb` for the background and the text, or empty to leave the site's own.
    #[serde(default)]
    pub bg: String,
    #[serde(default)]
    pub fg: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Boosts {
    items: Vec<Boost>,
}

fn clean_css(css: &str) -> String {
    css.chars()
        .filter(|c| !c.is_control() || matches!(c, '\n' | '\t'))
        .take(MAX_CSS)
        .collect()
}

/// The site a user typed: an address or a bare host.
pub fn host_of(text: &str) -> Option<String> {
    let t = text.trim();
    if t.starts_with("https://") || t.starts_with("http://") {
        return normalize_host(t);
    }
    // A bare host: no scheme, port or path to smuggle anything in.
    if t.contains([':', '/', '?', '#', '@']) {
        return None;
    }
    normalize_host(&format!("https://{t}/"))
}

impl Boosts {
    pub fn from_json(text: &str) -> Self {
        let mut b: Self = serde_json::from_str(text).unwrap_or_default();
        let mut seen: Vec<String> = Vec::new();
        b.items.retain_mut(|i| {
            let Some(host) = host_of(&i.host) else {
                return false;
            };
            i.host = host;
            i.css = clean_css(&i.css);
            i.hidden.retain(|h| valid_selector(h));
            i.hidden.dedup();
            i.hidden.truncate(MAX_HIDDEN);
            if !FONTS.iter().any(|f| f.0 == i.font) {
                i.font.clear();
            }
            if !valid_color(&i.bg) {
                i.bg.clear();
            }
            if !valid_color(&i.fg) {
                i.fg.clear();
            }
            if seen.contains(&i.host) {
                return false;
            }
            seen.push(i.host.clone());
            true
        });
        b.items.truncate(MAX_BOOSTS);
        b
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| "{}".to_owned())
    }

    pub fn items(&self) -> &[Boost] {
        &self.items
    }

    pub fn get(&self, host: &str) -> Option<&Boost> {
        self.items.iter().find(|b| b.host == host)
    }

    /// Starts a boost for a site; returns its host, or `None` for something that is not a site
    /// or when there are too many.
    pub fn add(&mut self, text: &str) -> Option<String> {
        let host = host_of(text)?;
        if self.get(&host).is_none() {
            if self.items.len() >= MAX_BOOSTS {
                return None;
            }
            self.items.push(Boost {
                host: host.clone(),
                dark: false,
                css: String::new(),
                hidden: Vec::new(),
                font: String::new(),
                bg: String::new(),
                fg: String::new(),
            });
        }
        Some(host)
    }

    /// Hides an element of the site (Zap). The boost is made if the site had none. `false` when the
    /// selector is not acceptable, was there already, or the site has too many.
    pub fn hide(&mut self, host: &str, selector: &str) -> bool {
        let selector = selector.trim();
        if !valid_selector(selector) || self.add(host).is_none() {
            return false;
        }
        match self.items.iter_mut().find(|b| b.host == host) {
            Some(b) if !b.hidden.iter().any(|h| h == selector) && b.hidden.len() < MAX_HIDDEN => {
                b.hidden.push(selector.to_owned());
                true
            }
            _ => false,
        }
    }

    /// Shows again the element at `index` of the site's hidden list.
    pub fn unhide(&mut self, host: &str, index: usize) -> bool {
        match self.items.iter_mut().find(|b| b.host == host) {
            Some(b) if index < b.hidden.len() => {
                b.hidden.remove(index);
                true
            }
            _ => false,
        }
    }

    pub fn unhide_all(&mut self, host: &str) -> bool {
        match self.items.iter_mut().find(|b| b.host == host) {
            Some(b) if !b.hidden.is_empty() => {
                b.hidden.clear();
                true
            }
            _ => false,
        }
    }

    /// Chooses one of `FONTS` (`""` leaves the site's own). Unknown ids are refused.
    pub fn set_font(&mut self, host: &str, font: &str) -> bool {
        if !font.is_empty() && !FONTS.iter().any(|f| f.0 == font) {
            return false;
        }
        match self.items.iter_mut().find(|b| b.host == host) {
            Some(b) if b.font != font => {
                b.font = font.to_owned();
                true
            }
            _ => false,
        }
    }

    /// Sets the background (`which` = `bg`) or the text colour (`fg`); `""` clears it.
    pub fn set_color(&mut self, host: &str, which: &str, color: &str) -> bool {
        if !color.is_empty() && !valid_color(color) {
            return false;
        }
        let Some(b) = self.items.iter_mut().find(|b| b.host == host) else {
            return false;
        };
        let slot = match which {
            "bg" => &mut b.bg,
            "fg" => &mut b.fg,
            _ => return false,
        };
        if slot == color {
            return false;
        }
        *slot = color.to_owned();
        true
    }

    pub fn remove(&mut self, host: &str) -> bool {
        let before = self.items.len();
        self.items.retain(|b| b.host != host);
        self.items.len() != before
    }

    pub fn set_dark(&mut self, host: &str, dark: bool) -> bool {
        match self.items.iter_mut().find(|b| b.host == host) {
            Some(b) if b.dark != dark => {
                b.dark = dark;
                true
            }
            _ => false,
        }
    }

    pub fn set_css(&mut self, host: &str, css: &str) -> bool {
        let css = clean_css(css);
        match self.items.iter_mut().find(|b| b.host == host) {
            Some(b) if b.css != css => {
                b.css = css;
                true
            }
            _ => false,
        }
    }
}

impl Boost {
    /// The style sheet to inject, or `None` when the boost does nothing.
    pub fn stylesheet(&self) -> Option<String> {
        let mut sheet = String::new();
        if self.dark {
            sheet.push_str(DARK);
        }
        if let Some((_, _, family)) = FONTS.iter().find(|f| f.0 == self.font) {
            // Icon fonts keep theirs: anything named like an icon is left alone.
            sheet.push_str(&format!(
                "html, body, body :not([class*=\"icon\" i]):not([class*=\"fa-\"]):not(i):not(code):not(pre):not(kbd):not(samp):not(svg):not(svg *) {{ font-family: {family} !important; }}\n"
            ));
        }
        if valid_color(&self.bg) || valid_color(&self.fg) {
            let mut rule = String::new();
            if valid_color(&self.bg) {
                rule.push_str(&format!("background-color: {} !important; ", self.bg));
            }
            if valid_color(&self.fg) {
                rule.push_str(&format!("color: {} !important; ", self.fg));
            }
            sheet.push_str(&format!("html, body {{ {}}}\n", rule));
        }
        // What was zapped, one rule each: a selector the browser cannot read only spoils its own.
        for selector in self.hidden.iter().filter(|h| valid_selector(h)) {
            sheet.push_str(&format!("{selector} {{ display: none !important; }}\n"));
        }
        sheet.push_str(&self.css);
        (!sheet.trim().is_empty()).then_some(sheet)
    }

    /// WebKit match patterns for the site and its subdomains.
    pub fn patterns(&self) -> Vec<String> {
        let h = &self.host;
        vec![
            format!("https://{h}/*"),
            format!("http://{h}/*"),
            format!("https://*.{h}/*"),
            format!("http://*.{h}/*"),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sites_are_taken_from_addresses_or_hosts_and_nothing_else() {
        assert_eq!(
            host_of("https://www.Example.com/a?b").as_deref(),
            Some("example.com")
        );
        assert_eq!(host_of("  example.com ").as_deref(), Some("example.com"));
        for bad in [
            "",
            "javascript:alert(1)",
            "a b",
            "exa*mple.com",
            "file:///x",
            "http://",
        ] {
            assert_eq!(host_of(bad), None, "{bad}");
        }
    }

    #[test]
    fn a_boost_is_added_once_and_edited() {
        let mut b = Boosts::default();
        assert_eq!(b.add("https://www.wiki.org/x").as_deref(), Some("wiki.org"));
        assert_eq!(b.add("wiki.org").as_deref(), Some("wiki.org"));
        assert_eq!(b.items().len(), 1);
        assert!(b.set_dark("wiki.org", true));
        assert!(!b.set_dark("wiki.org", true));
        assert!(b.set_css("wiki.org", "body{font-size:20px}"));
        assert!(!b.set_css("nowhere.org", "x"));
        let sheet = b.get("wiki.org").unwrap().stylesheet().unwrap();
        assert!(sheet.contains("invert(1)") && sheet.ends_with("font-size:20px}"));
        assert!(b.remove("wiki.org"));
        assert!(!b.remove("wiki.org"));
    }

    #[test]
    fn a_boost_that_changes_nothing_has_no_sheet() {
        let mut b = Boosts::default();
        b.add("a.org");
        assert_eq!(b.get("a.org").unwrap().stylesheet(), None);
        b.set_css("a.org", "  \n");
        assert_eq!(b.get("a.org").unwrap().stylesheet(), None);
    }

    #[test]
    fn zapped_elements_become_rules_and_can_be_shown_again() {
        let mut b = Boosts::default();
        assert!(b.hide("news.org", "div.banner > a:nth-of-type(2)"));
        assert!(!b.hide("news.org", "div.banner > a:nth-of-type(2)"), "once");
        assert!(b.hide("news.org", "#cookies"));
        let sheet = b.get("news.org").unwrap().stylesheet().unwrap();
        assert!(sheet.contains("div.banner > a:nth-of-type(2) { display: none !important; }"));
        assert!(sheet.contains("#cookies { display: none !important; }"));
        assert!(b.unhide("news.org", 0));
        assert!(!b.unhide("news.org", 5));
        assert_eq!(b.get("news.org").unwrap().hidden, ["#cookies"]);
        assert!(b.unhide_all("news.org"));
        assert!(!b.unhide_all("news.org"));
    }

    #[test]
    fn a_selector_cannot_close_the_rule_or_start_another() {
        for bad in [
            "",
            "a{color:red}",
            "a} body{display:none",
            "a;b",
            "a/*x*/",
            "@import url(x)",
            "a\\b",
            "<style>",
            "a !important",
            "a[href=\"x]",
            "a(b",
            &"a".repeat(MAX_SELECTOR + 1),
        ] {
            assert!(!valid_selector(bad), "{bad}");
            assert!(!Boosts::default().hide("x.org", bad), "{bad}");
        }
        for good in [
            "#id",
            "div.a > p:nth-of-type(3)",
            "[data-x=\"y\"]",
            "ul li + li",
            "a[href^='/x']",
        ] {
            assert!(valid_selector(good), "{good}");
        }
    }

    #[test]
    fn font_and_colours_come_from_fixed_choices_and_checked_values() {
        let mut b = Boosts::default();
        b.add("a.org");
        assert!(b.set_font("a.org", "serif"));
        assert!(!b.set_font("a.org", "Comic}"));
        assert!(b.set_color("a.org", "bg", "#101010"));
        assert!(b.set_color("a.org", "fg", "#eeeeee"));
        assert!(!b.set_color("a.org", "bg", "red"));
        assert!(!b.set_color("a.org", "bg", "#12345;"));
        assert!(!b.set_color("a.org", "other", "#101010"));
        let sheet = b.get("a.org").unwrap().stylesheet().unwrap();
        assert!(sheet.contains("font-family: Georgia"));
        assert!(sheet.contains("background-color: #101010 !important; color: #eeeeee !important;"));
        assert!(b.set_color("a.org", "bg", ""));
        assert!(b.set_font("a.org", ""));
        let sheet = b.get("a.org").unwrap().stylesheet().unwrap();
        assert!(!sheet.contains("font-family") && !sheet.contains("background-color"));
    }

    #[test]
    fn a_file_with_bad_values_is_cleaned_when_read() {
        let back = Boosts::from_json(
            r##"{"items":[{"host":"a.org","hidden":["#ok","a{x}","#ok"],"font":"nope","bg":"#12","fg":"#abcdef"}]}"##,
        );
        let b = back.get("a.org").unwrap();
        assert_eq!(b.hidden, ["#ok"]);
        assert_eq!(
            (b.font.as_str(), b.bg.as_str(), b.fg.as_str()),
            ("", "", "#abcdef")
        );
    }

    #[test]
    fn patterns_name_the_site_and_its_subdomains_only() {
        let mut b = Boosts::default();
        b.add("a.org");
        let p = b.get("a.org").unwrap().patterns();
        assert!(p.contains(&"https://a.org/*".to_owned()));
        assert!(p.contains(&"https://*.a.org/*".to_owned()));
        assert!(p.iter().all(|x| !x.starts_with("*") && x.ends_with("/*")));
    }

    #[test]
    fn css_is_cleaned_and_bounded_and_a_file_is_read_forgivingly() {
        let mut b = Boosts::default();
        b.add("a.org");
        b.set_css("a.org", &format!("a{{}}\u{0}{}", "x".repeat(MAX_CSS * 2)));
        let css = &b.get("a.org").unwrap().css;
        assert_eq!(css.chars().count(), MAX_CSS);
        assert!(!css.contains('\u{0}'));
        let back = Boosts::from_json(
            r#"{"items":[{"host":"b.org"},{"host":"b.org"},{"host":"bad host"},{"host":"https://c.org/"}]}"#,
        );
        let hosts: Vec<&str> = back.items().iter().map(|i| i.host.as_str()).collect();
        assert_eq!(hosts, ["b.org", "c.org"]);
        assert_eq!(Boosts::from_json("{no"), Boosts::default());
    }
}
