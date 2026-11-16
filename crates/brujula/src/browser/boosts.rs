//! Boosts: the look of a site as the user changed it, applied as user style sheets that only
//! match that site.

use super::*;

use brujula_core::boosts::{self, Boosts};
use webkit6::{UserContentInjectedFrames, UserStyleLevel, UserStyleSheet};

pub(super) fn load() -> Boosts {
    std::fs::read_to_string(paths::boosts_file())
        .map(|t| Boosts::from_json(&t))
        .unwrap_or_default()
}

impl Browser {
    fn save_boosts(&self) {
        if self.private {
            return;
        }
        let _ = paths::write_atomic(&paths::boosts_file(), &self.boosts.borrow().to_json());
    }

    /// Puts the style sheets of the boosts in the content manager, in place of the old ones.
    pub(super) fn apply_boosts(&self) {
        let mut sheets = self.boost_sheets.borrow_mut();
        for old in sheets.drain(..) {
            self.ucm.remove_style_sheet(&old);
        }
        for boost in self.boosts.borrow().items() {
            let Some(css) = boost.stylesheet() else {
                continue;
            };
            let patterns = boost.patterns();
            let allow: Vec<&str> = patterns.iter().map(String::as_str).collect();
            let sheet = UserStyleSheet::new(
                &css,
                UserContentInjectedFrames::AllFrames,
                UserStyleLevel::User,
                &allow,
                &[],
            );
            self.ucm.add_style_sheet(&sheet);
            sheets.push(sheet);
        }
    }

    /// Loads again the open pages of a site, so a change to its boost shows.
    fn reload_site(&self, host: &str) {
        let views: Vec<WebView> = self.views.borrow().values().cloned().collect();
        for v in views {
            let same = v
                .uri()
                .and_then(|u| adblock::normalize_host(&u))
                .is_some_and(|h| h == host || h.ends_with(&format!(".{host}")));
            if same {
                v.reload();
            }
        }
    }

    /// A control of the Boosts section: `add`, `remove`, `dark.<site>` or `css.<site>`.
    pub(super) fn change_boost(self: &Rc<Self>, what: &str, value: &str) {
        let site = |key: &str| what.strip_prefix(key).map(str::to_owned);
        let host = if what == "add" {
            let added = self.boosts.borrow_mut().add(value);
            if added.is_some() {
                // The page lists the new site: draw it again once this handler is done, and
                // come back to this section.
                *self.pending_section.borrow_mut() = Some("boosts".to_owned());
                let this = Rc::downgrade(self);
                glib::idle_add_local_once(move || {
                    if let Some(b) = this.upgrade() {
                        b.redraw_settings();
                    }
                });
            }
            None
        } else if what == "remove" {
            let host = boosts::host_of(value);
            if let Some(h) = &host {
                self.boosts.borrow_mut().remove(h);
            }
            host
        } else if let Some(h) = site("dark.") {
            let on = matches!(value, "1" | "true" | "on");
            self.boosts.borrow_mut().set_dark(&h, on).then_some(h)
        } else if let Some(h) = site("css.") {
            self.boosts.borrow_mut().set_css(&h, value).then_some(h)
        } else if let Some(h) = site("font.") {
            self.boosts.borrow_mut().set_font(&h, value).then_some(h)
        } else if let Some(h) = site("bg.") {
            self.boosts
                .borrow_mut()
                .set_color(&h, "bg", value)
                .then_some(h)
        } else if let Some(h) = site("fg.") {
            self.boosts
                .borrow_mut()
                .set_color(&h, "fg", value)
                .then_some(h)
        } else if let Some(h) = site("colors.") {
            let mut boosts = self.boosts.borrow_mut();
            let a = boosts.set_color(&h, "bg", "");
            let b = boosts.set_color(&h, "fg", "");
            (a || b).then_some(h)
        } else if let Some(h) = site("unhide_all.") {
            self.boosts.borrow_mut().unhide_all(&h).then_some(h)
        } else if let Some(h) = site("unhide.") {
            let index = value.parse().ok();
            index
                .is_some_and(|i| self.boosts.borrow_mut().unhide(&h, i))
                .then_some(h)
        } else {
            return;
        };
        self.save_boosts();
        self.apply_boosts();
        if let Some(h) = host {
            self.reload_site(&h);
        }
    }

    /// Draws the preferences page again where it was scrolled to, for changes that alter what the
    /// page shows (a gradient losing its third colour, appearance back to the defaults).
    pub(super) fn redraw_settings(self: &Rc<Self>) {
        let view = self
            .views
            .borrow()
            .values()
            .find(|v| v.uri().as_deref() == Some(prefs::SETTINGS_URI))
            .cloned();
        let Some(view) = view else { return };
        let this = self.clone();
        let v = view.clone();
        view.evaluate_javascript(
            "(document.querySelector('main>section.on')?.id ?? '') + ' ' + document.scrollingElement.scrollTop",
            None,
            None,
            gio::Cancellable::NONE,
            move |value| {
                // The page shows one section at a time: it comes back to that one, at that height.
                let state = value.ok().map(|j| j.to_str().to_string()).unwrap_or_default();
                let (section, top) = state.split_once(' ').unwrap_or(("", "0"));
                // A section asked for before the redraw (the new boost's, say) wins over this
                // one, and the height of the old one means nothing there.
                let mut wanted = this.pending_section.borrow_mut();
                if wanted.is_some() {
                    this.pending_scroll.set(None);
                } else {
                    if !section.is_empty() && section.chars().all(|c| c.is_ascii_alphanumeric()) {
                        *wanted = Some(section.to_owned());
                    }
                    this.pending_scroll.set(top.parse().ok());
                }
                drop(wanted);
                this.render_settings(&v);
            },
        );
    }

    /// Starts a boost for the site of the tab and shows it in the preferences.
    pub(super) fn boost_site_of(self: &Rc<Self>, id: TabId) {
        let host = self
            .model
            .borrow()
            .get(id)
            .and_then(|t| adblock::normalize_host(&t.url));
        let Some(host) = host else { return };
        self.boosts.borrow_mut().add(&host);
        self.save_boosts();
        self.open_settings(Some("boosts"));
        self.redraw_settings();
    }
}

const ZAP_JS: &str = include_str!("zap.js");
const ZAP_HANDLER: &str = "brujulaZap";
const ZAP_WORLD: &str = "brujula-zap";

impl Browser {
    /// Registers what Zap's page script talks to. The script runs in a world of its own that the
    /// page cannot reach.
    pub(super) fn start_zap(self: &Rc<Self>) {
        if self.private {
            return;
        }
        self.ucm
            .register_script_message_handler(ZAP_HANDLER, Some(ZAP_WORLD));
        let this = Rc::downgrade(self);
        self.ucm
            .connect_script_message_received(Some(ZAP_HANDLER), move |_, value| {
                if let Some(b) = this.upgrade() {
                    b.zapped(&value.to_str());
                }
            });
    }

    /// «Ocultar un elemento de la página»: the page in front lets the user click what to hide.
    pub(super) fn start_zapping(self: &Rc<Self>) {
        let Some(view) = self.current_view() else {
            return;
        };
        let host = view
            .uri()
            .filter(|u| u.starts_with("http://") || u.starts_with("https://"))
            .and_then(|u| adblock::normalize_host(&u));
        let Some(host) = host.filter(|_| !self.private) else {
            self.toast.flash(
                "ndl-close-symbolic",
                "Solo se pueden ocultar elementos de páginas web, y no en una ventana privada",
            );
            return;
        };
        *self.zap_host.borrow_mut() = Some(host);
        // The script lives in its own world, which the page cannot reach.
        view.evaluate_javascript(
            ZAP_JS,
            Some(ZAP_WORLD),
            None,
            gio::Cancellable::NONE,
            |_| {},
        );
    }

    /// The page says which element the user picked: it goes into the boost of the site that was
    /// in front when Zap started.
    fn zapped(self: &Rc<Self>, selector: &str) {
        let Some(host) = self.zap_host.borrow_mut().take() else {
            return;
        };
        if !self.boosts.borrow_mut().hide(&host, selector) {
            self.toast
                .flash("ndl-close-symbolic", "No se pudo ocultar ese elemento");
            return;
        }
        self.save_boosts();
        self.apply_boosts();
        let this = Rc::downgrade(self);
        let selector = selector.trim().to_owned();
        self.toast.show(
            None,
            "ndl-check-symbolic",
            &format!("Elemento oculto en {host}"),
            vec![(
                "Deshacer".to_owned(),
                true,
                Box::new(move || {
                    let Some(b) = this.upgrade() else { return };
                    let index = b
                        .boosts
                        .borrow()
                        .get(&host)
                        .and_then(|boost| boost.hidden.iter().position(|h| *h == selector));
                    if let Some(i) = index {
                        b.boosts.borrow_mut().unhide(&host, i);
                        b.save_boosts();
                        b.apply_boosts();
                        b.reload_site(&host);
                    }
                }),
            )],
        );
    }
}
