//! Little Arc: a link from another application, with Brújula closed, opens in a small window
//! with just the page instead of loading the whole browser in front of it. «Abrir en Brújula»
//! takes the page to a tab of the full window; closing the small window quits when the full
//! window was never shown.
//!
//! The page is as locked down as a tab's (`card_view`: same network session, same blocker, no
//! permissions, no new windows, only web addresses). With Brújula already open the link goes to
//! Peek, as before.

use super::*;

/// The small window and its page.
pub(super) struct MiniWindow {
    window: gtk::ApplicationWindow,
    view: WebView,
}

fn is_web(uri: &str) -> bool {
    uri.starts_with("https://") || uri.starts_with("http://")
}

impl Browser {
    /// Whether a link that arrives now should go to the small window: Brújula is not showing
    /// its full window (it was launched for this link, or only the small one is open).
    pub(super) fn wants_mini(&self, urls: &[String]) -> bool {
        self.settings.borrow().quick_links
            && !self.private
            && !self.window.is_visible()
            && urls.len() == 1
            && is_web(&urls[0])
    }

    /// Shows `uri` in the small window, opening it if it is not there.
    pub(super) fn open_mini(self: &Rc<Self>, uri: &str) {
        if !is_web(uri) {
            return;
        }
        if let Some(mini) = self.mini.borrow().as_ref() {
            mini.view.load_uri(uri);
            mini.window.present();
            return;
        }
        let Some(app) = self.window.application() else {
            return;
        };
        let view = self.card_view();
        let title = gtk::Label::builder()
            .xalign(0.0)
            .hexpand(true)
            .ellipsize(gtk::pango::EllipsizeMode::End)
            .label(uri)
            .build();
        let open = gtk::Button::builder()
            .label("Abrir en Brújula")
            .css_classes(["flat"])
            .build();
        let close = gtk::Button::builder()
            .icon_name("ndl-close-symbolic")
            .tooltip_text("Cerrar (Esc)")
            .css_classes(["flat"])
            .build();
        let header = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        header.add_css_class("brujula-peek-header");
        header.append(&title);
        header.append(&open);
        header.append(&close);
        let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
        content.append(&header);
        content.append(&view);
        let window = gtk::ApplicationWindow::builder()
            .application(&app)
            .title("Brújula")
            .default_width(560)
            .default_height(760)
            .child(&content)
            .build();
        window.add_css_class("brujula-mini");

        let weak = Rc::downgrade(self);
        view.connect_title_notify({
            let title = title.clone();
            move |v| {
                let text = v.title().map(|t| t.to_string()).unwrap_or_default();
                let shown = if text.is_empty() {
                    v.uri().map(|u| u.to_string()).unwrap_or_default()
                } else {
                    text
                };
                title.set_text(&shown);
            }
        });
        view.connect_load_changed({
            let weak = weak.clone();
            move |v, ev| {
                if let (Some(b), LoadEvent::Finished) = (weak.upgrade(), ev) {
                    hide_elements(&b.filters, v);
                }
            }
        });
        close.connect_clicked({
            let window = window.clone();
            move |_| window.close()
        });
        open.connect_clicked({
            let weak = weak.clone();
            move |_| {
                if let Some(b) = weak.upgrade() {
                    b.keep_mini();
                }
            }
        });
        // Esc or Ctrl+W close it, whatever has the focus.
        let key = gtk::EventControllerKey::new();
        key.set_propagation_phase(gtk::PropagationPhase::Capture);
        key.connect_key_pressed({
            let window = window.clone();
            move |_, keyval, _, state| {
                let ctrl_w =
                    keyval == gdk::Key::w && state.contains(gdk::ModifierType::CONTROL_MASK);
                if keyval == gdk::Key::Escape || ctrl_w {
                    window.close();
                    glib::Propagation::Stop
                } else {
                    glib::Propagation::Proceed
                }
            }
        });
        window.add_controller(key);
        window.connect_close_request({
            let weak = weak.clone();
            move |_| {
                if let Some(b) = weak.upgrade() {
                    b.mini.borrow_mut().take();
                    // Launched only for this link: nothing else is on show, so Brújula ends.
                    if !b.window.is_visible() {
                        if let Some(app) = b.window.application() {
                            glib::idle_add_local_once(move || app.quit());
                        }
                    }
                }
                glib::Propagation::Proceed
            }
        });
        view.load_uri(uri);
        window.present();
        view.grab_focus();
        *self.mini.borrow_mut() = Some(MiniWindow { window, view });
    }

    /// «Abrir en Brújula»: the page goes to a tab of the full window.
    fn keep_mini(self: &Rc<Self>) {
        let taken = self.mini.borrow_mut().take();
        let Some(mini) = taken else { return };
        let uri = mini.view.uri().map(|u| u.to_string());
        mini.window.close();
        self.window.present();
        if let Some(uri) = uri.filter(|u| is_web(u)) {
            self.open_tab(&uri, None, true);
        }
    }
}
