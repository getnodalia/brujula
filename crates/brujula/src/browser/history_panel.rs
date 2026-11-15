//! The library button: it slides the library in over the list of tabs in the sidebar (and back),
//! or, with the tabs on top, drops it from the bar as a list. The history is the first of its
//! sections, beside the downloads, the screenshots and the archived tabs (Arc's Library): a click
//! on a page opens it in a new tab, on a file opens it with its application. What the history
//! keeps and how it is cleaned is in `history.rs`; the files are picked in `library.rs`.

use super::*;

/// How many pages the list shows.
const SHOWN: usize = 200;
/// How many files the downloads and the screenshots show.
const SHOWN_FILES: usize = 60;

/// What a row of the library does when it is clicked.
#[derive(Clone)]
enum Target {
    /// A page of the history: opens in a new tab.
    Page(String),
    /// A download or a screenshot: opens with the application of its type.
    File(std::path::PathBuf),
    /// An archived tab: comes back in the space in front.
    Archived(String),
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Section {
    History,
    Downloads,
    Captures,
    Archive,
}

const SECTIONS: [(Section, &str); 4] = [
    (Section::History, "Historial"),
    (Section::Downloads, "Descargas"),
    (Section::Captures, "Capturas"),
    (Section::Archive, "Archivo"),
];

/// The widgets of the history list.
pub(super) struct HistoryUi {
    /// Sidebar: the button at the bottom, which slides the history in.
    pub side_button: gtk::ToggleButton,
    /// Tabs on top: the button of the bar, which drops the history as a list.
    pub top_button: gtk::MenuButton,
    /// The tabs and the history, one over the other in the sidebar.
    pub stack: gtk::Stack,
    pane: gtk::Box,
    popover: gtk::Popover,
    list: gtk::ListBox,
    search: gtk::SearchEntry,
    empty: gtk::Label,
    back: gtk::Button,
    /// What each row of the list does (`None` for a heading).
    targets: RefCell<Vec<Option<Target>>>,
    /// The section on show, and the buttons that choose it.
    section: Cell<Section>,
    choosers: Vec<(Section, gtk::ToggleButton)>,
}

fn history_icon() -> &'static str {
    if nodalia_gtk::has_icon("ndl-history-symbolic") {
        "ndl-history-symbolic"
    } else {
        "document-open-recent-symbolic"
    }
}

/// «Hoy», «Ayer» or the date, for the heading over the pages of a day.
fn day_label(last_ms: u64) -> String {
    let (Ok(then), Ok(now)) = (
        glib::DateTime::from_unix_local(i64::try_from(last_ms / 1000).unwrap_or(0)),
        glib::DateTime::now_local(),
    ) else {
        return String::new();
    };
    if then.ymd() == now.ymd() {
        return "Hoy".to_owned();
    }
    if now
        .add_days(-1)
        .is_ok_and(|yesterday| yesterday.ymd() == then.ymd())
    {
        return "Ayer".to_owned();
    }
    then.format("%d/%m/%Y")
        .map(|s| s.to_string())
        .unwrap_or_default()
}

impl HistoryUi {
    /// `tabs` is the list of tabs, which this puts in the stack as its first page.
    pub fn new(tabs: &impl IsA<gtk::Widget>) -> Self {
        let stack = gtk::Stack::builder()
            .transition_type(gtk::StackTransitionType::SlideLeftRight)
            .transition_duration(220)
            .vexpand(true)
            .build();
        stack.add_named(tabs, Some("tabs"));

        let back = gtk::Button::builder()
            .icon_name("ndl-arrow-left-symbolic")
            .css_classes(["flat"])
            .tooltip_text("Volver a las pestañas")
            .build();
        let title = gtk::Label::builder()
            .label("Biblioteca")
            .xalign(0.0)
            .hexpand(true)
            .css_classes(["title-4"])
            .build();
        let head = gtk::Box::new(gtk::Orientation::Horizontal, 4);
        head.set_margin_start(4);
        head.set_margin_end(8);
        head.set_margin_bottom(4);
        head.append(&back);
        head.append(&title);
        // One button per section, linked: choosing one shows its list.
        let chooser_row = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        chooser_row.add_css_class("linked");
        chooser_row.set_margin_start(8);
        chooser_row.set_margin_end(8);
        chooser_row.set_margin_bottom(6);
        let mut choosers: Vec<(Section, gtk::ToggleButton)> = Vec::new();
        for (section, label) in SECTIONS {
            let button = gtk::ToggleButton::builder()
                .label(label)
                .hexpand(true)
                .active(section == Section::History)
                .css_classes(["flat", "brujula-library-tab"])
                .build();
            if let Some((_, first)) = choosers.first() {
                button.set_group(Some(first));
            }
            chooser_row.append(&button);
            choosers.push((section, button));
        }
        let search = gtk::SearchEntry::builder()
            .placeholder_text("Buscar en el historial")
            .margin_start(8)
            .margin_end(8)
            .margin_bottom(6)
            .build();
        let list = gtk::ListBox::new();
        list.set_selection_mode(gtk::SelectionMode::None);
        list.add_css_class("navigation-sidebar");
        list.add_css_class("brujula-history-list");
        list.set_activate_on_single_click(true);
        let scroller = gtk::ScrolledWindow::builder()
            .child(&list)
            .vexpand(true)
            .margin_start(8)
            .margin_end(8)
            .hscrollbar_policy(gtk::PolicyType::Never)
            .build();
        let empty = gtk::Label::builder()
            .label("No hay nada en el historial.")
            .css_classes(["dim-label"])
            .wrap(true)
            .max_width_chars(1)
            .margin_top(18)
            .margin_bottom(18)
            .build();
        let pane = gtk::Box::new(gtk::Orientation::Vertical, 0);
        pane.add_css_class("brujula-history");
        pane.append(&head);
        pane.append(&chooser_row);
        pane.append(&search);
        pane.append(&empty);
        pane.append(&scroller);
        stack.add_named(&pane, Some("history"));

        let side_button = gtk::ToggleButton::builder()
            .icon_name(history_icon())
            .tooltip_text("Biblioteca (Ctrl+H)")
            .css_classes(["flat", "brujula-newfolder"])
            .build();
        let popover = gtk::Popover::new();
        popover.add_css_class("brujula-history-pop");
        let top_button = gtk::MenuButton::builder()
            .icon_name(history_icon())
            .tooltip_text("Biblioteca (Ctrl+H)")
            .css_classes(["flat", "brujula-newfolder"])
            .popover(&popover)
            .visible(false)
            .build();
        Self {
            side_button,
            top_button,
            stack,
            pane,
            popover,
            list,
            search,
            empty,
            back,
            targets: RefCell::new(Vec::new()),
            section: Cell::new(Section::History),
            choosers,
        }
    }

    /// Whether the history list is showing, in the sidebar or in the popover.
    fn open(&self) -> bool {
        self.side_button.is_active() || self.popover.is_visible()
    }
}

impl Browser {
    pub(super) fn wire_history_panel(self: &Rc<Self>) {
        let this = Rc::downgrade(self);
        self.hist.side_button.connect_toggled(move |button| {
            let Some(b) = this.upgrade() else { return };
            b.hist.stack.set_visible_child_name(if button.is_active() {
                "history"
            } else {
                "tabs"
            });
            if button.is_active() {
                b.refill_history();
                b.hist.search.grab_focus();
            }
        });
        let button = self.hist.side_button.clone();
        self.hist.back.connect_clicked(move |_| {
            button.set_active(false);
        });
        let popover = self.hist.popover.clone();
        self.hist.back.connect_clicked(move |_| popover.popdown());
        let this = Rc::downgrade(self);
        self.hist.popover.connect_show(move |_| {
            if let Some(b) = this.upgrade() {
                b.refill_history();
                b.hist.search.grab_focus();
            }
        });
        let this = Rc::downgrade(self);
        self.hist.search.connect_search_changed(move |_| {
            if let Some(b) = this.upgrade() {
                b.refill_history();
            }
        });
        for (section, button) in &self.hist.choosers {
            let this = Rc::downgrade(self);
            let section = *section;
            button.connect_toggled(move |button| {
                if !button.is_active() {
                    return;
                }
                if let Some(b) = this.upgrade() {
                    b.hist.section.set(section);
                    b.hist.search.set_visible(section == Section::History);
                    b.refill_history();
                }
            });
        }
        let this = Rc::downgrade(self);
        self.hist.list.connect_row_activated(move |_, row| {
            let Some(b) = this.upgrade() else { return };
            let target = usize::try_from(row.index())
                .ok()
                .and_then(|i| b.hist.targets.borrow().get(i).cloned().flatten());
            let Some(target) = target else { return };
            b.hist.side_button.set_active(false);
            b.hist.popover.popdown();
            match target {
                Target::Page(url) => {
                    b.open_tab(&url, None, true);
                }
                Target::Archived(url) => b.change_archive("open", &url),
                Target::File(path) => {
                    let uri = gio::File::for_path(&path).uri();
                    let _ = gio::AppInfo::launch_default_for_uri(&uri, gio::AppLaunchContext::NONE);
                }
            }
        });
        self.place_history(self.horizontal());
    }

    /// The history goes where the layout wants it: over the tabs in the sidebar, or in the
    /// popover of the bar.
    pub(super) fn place_history(&self, horizontal: bool) {
        let ui = &self.hist;
        ui.side_button.set_visible(!horizontal);
        ui.top_button.set_visible(horizontal);
        let in_stack = ui.pane.parent().as_ref() == Some(ui.stack.upcast_ref());
        if horizontal && in_stack {
            ui.side_button.set_active(false);
            ui.stack.set_visible_child_name("tabs");
            ui.stack.remove(&ui.pane);
            ui.pane.set_size_request(340, 420);
            ui.popover.set_child(Some(&ui.pane));
        } else if !horizontal && !in_stack {
            ui.popover.popdown();
            ui.popover.set_child(None::<&gtk::Widget>);
            ui.pane.set_size_request(-1, -1);
            ui.stack.add_named(&ui.pane, Some("history"));
        }
    }

    /// Ctrl+H: shows the history or goes back to the tabs.
    pub(super) fn toggle_history(self: &Rc<Self>) {
        if self.horizontal() {
            if self.hist.popover.is_visible() {
                self.hist.popover.popdown();
            } else {
                self.hist.top_button.popup();
            }
        } else {
            let on = !self.hist.side_button.is_active();
            self.hist.side_button.set_active(on);
        }
    }

    /// Fills the list with the pages that match what is typed in the search, newest first, under
    /// a heading for each day.
    fn refill_history(&self) {
        let ui = &self.hist;
        while let Some(row) = ui.list.first_child() {
            ui.list.remove(&row);
        }
        match ui.section.get() {
            Section::History => self.fill_pages(),
            Section::Downloads => self.fill_files(
                glib::user_special_dir(glib::UserDirectory::Downloads)
                    .unwrap_or_else(glib::home_dir),
                brujula_core::library::is_download,
                "No hay descargas en la carpeta Descargas.",
                false,
            ),
            Section::Captures => self.fill_files(
                glib::user_special_dir(glib::UserDirectory::Pictures)
                    .unwrap_or_else(glib::home_dir),
                brujula_core::library::is_capture,
                "Aún no has capturado ninguna página (Ctrl+Mayús+S).",
                true,
            ),
            Section::Archive => self.fill_archive(),
        }
    }

    /// The downloads and the screenshots: the newest files of their folder.
    fn fill_files(
        &self,
        dir: std::path::PathBuf,
        keep: fn(&str) -> bool,
        none: &str,
        thumbnails: bool,
    ) {
        let ui = &self.hist;
        let files = brujula_core::library::recent_files(&dir, keep, SHOWN_FILES);
        let mut targets = Vec::with_capacity(files.len());
        for f in &files {
            ui.list.append(&file_row(
                &f.name,
                &day_label(f.modified),
                &f.path,
                thumbnails,
            ));
            targets.push(Some(Target::File(f.path.clone())));
        }
        ui.empty.set_visible(files.is_empty());
        ui.empty.set_label(none);
        *ui.targets.borrow_mut() = targets;
    }

    /// The tabs the archive keeps, newest first.
    fn fill_archive(&self) {
        let ui = &self.hist;
        let entries: Vec<_> = self.archive.borrow().items().to_vec();
        let mut targets = Vec::with_capacity(entries.len());
        for e in &entries {
            let visit = brujula_core::history::Visit {
                url: e.url.clone(),
                title: e.title.clone(),
                last: e.at,
                visits: 1,
            };
            ui.list.append(&history_row(&visit));
            targets.push(Some(Target::Archived(e.url.clone())));
        }
        ui.empty.set_visible(entries.is_empty());
        ui.empty.set_label("No hay pestañas archivadas.");
        *ui.targets.borrow_mut() = targets;
    }

    /// The pages of the history that match what is typed in the search, newest first, under a
    /// heading for each day.
    fn fill_pages(&self) {
        let ui = &self.hist;
        let query = ui.search.text().to_string();
        let visits: Vec<_> = {
            let h = self.history.borrow();
            if query.trim().is_empty() {
                h.items().iter().take(SHOWN).cloned().collect()
            } else {
                h.search(&query, SHOWN).into_iter().cloned().collect()
            }
        };
        let mut targets = Vec::with_capacity(visits.len());
        let mut day = String::new();
        for visit in &visits {
            let label = day_label(visit.last);
            if label != day {
                day = label;
                let heading = gtk::Label::builder()
                    .label(&day)
                    .xalign(0.0)
                    .margin_start(8)
                    .margin_top(10)
                    .margin_bottom(2)
                    .css_classes(["dim-label", "caption-heading"])
                    .build();
                let row = gtk::ListBoxRow::new();
                row.set_activatable(false);
                row.set_selectable(false);
                row.set_child(Some(&heading));
                ui.list.append(&row);
                targets.push(None);
            }
            ui.list.append(&history_row(visit));
            targets.push(Some(Target::Page(visit.url.clone())));
        }
        ui.empty.set_visible(visits.is_empty());
        ui.empty.set_label(if query.trim().is_empty() {
            "No hay nada en el historial."
        } else {
            "No hay nada que coincida."
        });
        *ui.targets.borrow_mut() = targets;
    }

    /// The history changed (a page was visited, cleaned): redraws the list if it is showing.
    pub(super) fn history_changed(&self) {
        if self.hist.open() {
            self.refill_history();
        }
    }
}

fn history_row(visit: &brujula_core::history::Visit) -> gtk::ListBoxRow {
    let title = if visit.title.is_empty() {
        host_of(&visit.url)
    } else {
        visit.title.clone()
    };
    let name = gtk::Label::builder()
        .label(&title)
        .xalign(0.0)
        .ellipsize(gtk::pango::EllipsizeMode::End)
        .css_classes(["brujula-tab-title"])
        .build();
    let host = gtk::Label::builder()
        .label(host_of(&visit.url))
        .xalign(0.0)
        .ellipsize(gtk::pango::EllipsizeMode::End)
        .css_classes(["dim-label", "caption"])
        .build();
    let col = gtk::Box::new(gtk::Orientation::Vertical, 1);
    col.set_margin_start(8);
    col.set_margin_end(8);
    col.set_margin_top(5);
    col.set_margin_bottom(5);
    col.append(&name);
    col.append(&host);
    let row = gtk::ListBoxRow::new();
    row.add_css_class("brujula-row");
    row.set_tooltip_text(Some(&visit.url));
    row.set_child(Some(&col));
    row
}

/// A file of the downloads or the screenshots: its name, when it was made, and, for a screenshot,
/// a small preview.
fn file_row(name: &str, when: &str, path: &std::path::Path, thumbnail: bool) -> gtk::ListBoxRow {
    let title = gtk::Label::builder()
        .label(name)
        .xalign(0.0)
        .ellipsize(gtk::pango::EllipsizeMode::Middle)
        .css_classes(["brujula-tab-title"])
        .build();
    let detail = gtk::Label::builder()
        .label(when)
        .xalign(0.0)
        .css_classes(["dim-label", "caption"])
        .build();
    let col = gtk::Box::new(gtk::Orientation::Vertical, 1);
    col.set_hexpand(true);
    col.append(&title);
    col.append(&detail);
    let line = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    line.set_margin_start(8);
    line.set_margin_end(8);
    line.set_margin_top(5);
    line.set_margin_bottom(5);
    if thumbnail {
        let pic = gtk::Picture::for_filename(path);
        pic.set_content_fit(gtk::ContentFit::Cover);
        pic.set_can_shrink(true);
        pic.set_size_request(56, 36);
        pic.set_overflow(gtk::Overflow::Hidden);
        pic.add_css_class("brujula-library-thumb");
        line.append(&pic);
    } else {
        let icon = gtk::Image::from_icon_name("ndl-file-symbolic");
        icon.set_pixel_size(16);
        line.append(&icon);
    }
    line.append(&col);
    let row = gtk::ListBoxRow::new();
    row.add_css_class("brujula-row");
    row.set_tooltip_text(Some(&path.to_string_lossy()));
    row.set_child(Some(&line));
    row
}
