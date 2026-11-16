//! Pinned tabs and essentials that come back to their page, as in Arc: a pinned tab that has
//! wandered off shows a small button to return to the page it was pinned on, and «Usar esta
//! página como la anclada» moves that page. The same action (Alt+Inicio, or «Volver a la página
//! anclada» in the Spotlight) returns the page in front, pinned or an essential's.

use super::*;

impl Browser {
    /// The page the tab would return to, when it is not on it: the page a pinned tab was pinned
    /// on, or the front door of an essential.
    fn page_to_return_to(&self, id: TabId) -> Option<String> {
        let model = self.model.borrow();
        let tab = model.get(id)?;
        if let Some(e) = tab.essential {
            let home = self.essentials.borrow().get(e).map(|e| e.url.clone())?;
            return (!brujula_core::tabs::same_page(&home, &tab.url)).then_some(home);
        }
        model.home_to_return_to(id).map(str::to_owned)
    }

    /// Whether `id` shows a button to return: it is pinned or an essential and has left its page.
    pub(super) fn can_return_home(&self, id: TabId) -> bool {
        self.page_to_return_to(id).is_some()
    }

    /// Takes the tab back to its page.
    pub(super) fn return_to_pinned(self: &Rc<Self>, id: TabId) {
        let Some(home) = self.page_to_return_to(id) else {
            return;
        };
        let view = self.views.borrow().get(&id).cloned();
        if let Some(view) = view {
            view.load_uri(&home);
        }
    }

    /// The action behind the shortcut and the Spotlight command: the page in front returns.
    pub(super) fn return_current_to_pinned(self: &Rc<Self>) {
        let front = self.model.borrow().selected();
        match front {
            Some(id) if self.can_return_home(id) => self.return_to_pinned(id),
            Some(_) => self.toast.flash(
                "ndl-pin-symbolic",
                "Esta pestaña ya está en su página, o no está anclada",
            ),
            None => {}
        }
    }

    /// «Usar esta página como la anclada»: the page the tab shows now is the one it returns to.
    pub(super) fn set_pinned_page_here(self: &Rc<Self>, id: TabId) {
        let changes = self.model.borrow_mut().set_home_here(id);
        self.apply(changes);
        self.sync_row(id);
        self.schedule_save();
    }

    /// The button of a row that takes the tab back. It is only shown while the tab is away.
    pub(super) fn make_home_button(self: &Rc<Self>, id: TabId) -> gtk::Button {
        let home = gtk::Button::builder()
            .icon_name("ndl-home-symbolic")
            .css_classes(["flat", "circular", "brujula-tab-home"])
            .tooltip_text("Volver a la página anclada")
            .valign(gtk::Align::Center)
            .visible(false)
            .build();
        let this = Rc::downgrade(self);
        home.connect_clicked(move |_| {
            if let Some(b) = this.upgrade() {
                b.return_to_pinned(id);
            }
        });
        home
    }
}
