//! Link routing (Arc's Air Traffic Control): the links that arrive from other applications, or
//! that are kept from Peek or the small window, open in the space the rule of their site names.
//! The rules are in `routes.rs` of the core and are set in Preferencias → Enlaces.

use super::*;

use brujula_core::routes::Routes;

pub(super) fn load() -> Routes {
    std::fs::read_to_string(paths::routes_file())
        .map(|t| Routes::from_json(&t))
        .unwrap_or_default()
}

impl Browser {
    fn save_routes(&self) {
        if self.private {
            return;
        }
        let _ = paths::write_atomic(&paths::routes_file(), &self.routes.borrow().to_json());
    }

    /// The space a link to `url` is routed to, if a rule says so.
    pub(super) fn routed_space(&self, url: &str) -> Option<u32> {
        if self.private {
            return None;
        }
        self.routes.borrow().space_for(url, &self.spaces.borrow())
    }

    /// A link that comes from outside the page in front: in the space its rule names (which
    /// comes to the front, with a note), or where the user is.
    pub(super) fn open_external(self: &Rc<Self>, url: &str) -> TabId {
        let Some(space) = self.routed_space(url) else {
            return self.open_tab(url, None, true);
        };
        self.switch_space(space);
        let id = self.open_tab(url, None, true);
        let name = self
            .spaces
            .borrow()
            .get(space)
            .map(|s| s.name.clone())
            .unwrap_or_default();
        self.toast
            .flash("ndl-link-symbolic", &format!("Enlace abierto en {name}"));
        id
    }

    /// One of the `route.*` changes of the preferences page: `add` (`site`, a newline and the id
    /// of the space) or `remove` (the site).
    pub(super) fn change_route(self: &Rc<Self>, what: &str, value: &str) {
        if self.private {
            return;
        }
        match what {
            "add" => {
                let (site, space) = value.split_once('\n').unwrap_or((value, ""));
                let Ok(space) = space.trim().parse::<u32>() else {
                    return;
                };
                let added = self
                    .routes
                    .borrow_mut()
                    .set(site, space, &self.spaces.borrow());
                if added {
                    self.save_routes();
                    *self.pending_section.borrow_mut() = Some("routes".to_owned());
                    self.redraw_settings();
                } else {
                    self.essential_alert(
                        "No se pudo añadir la regla",
                        "El sitio no es válido o ya hay el máximo de reglas.",
                    );
                }
            }
            "remove" if self.routes.borrow_mut().remove(value) => self.save_routes(),
            _ => {}
        }
    }

    /// The rules as the preferences page lists them.
    pub(super) fn route_rules(&self) -> Vec<(String, u32)> {
        self.routes
            .borrow()
            .items()
            .iter()
            .map(|r| (r.site.clone(), r.space))
            .collect()
    }
}
