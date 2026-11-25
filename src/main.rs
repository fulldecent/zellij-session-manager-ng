//! A popup for the sessions on this machine.
//!
//! Find lists running sessions and saved sessions. Enter attaches, resurrects,
//! or opens a layout list for a new name. The breadcrumb is the pane title.

use std::collections::BTreeMap;
use std::io::{self, Write};

use zellij_tile::prelude::*;

use crate::chrome::{chrome_from, Chrome};
use crate::controls::Click;
use crate::line::{Edit, Line};
use crate::model::{filter, layout_label, sort_sessions, Found, Kind, Session};
use crate::paint::{kill_bars, new_bars, paint_list, paint_page, rename_bars, ListView, Pointer};

mod chrome;
mod controls;
mod line;
mod model;
mod paint;

const GRANT: &str = "Grant Read Application State and Change Application State.";

struct State {
    sessions: Vec<Session>,
    found: Vec<Found>,
    selected: Option<usize>,
    finding: bool,
    query: Line,
    layouts: Vec<LayoutInfo>,
    layout_index: usize,
    current_name: Option<String>,
    page: Page,
    granted: bool,
    notice: Option<String>,
    chrome: Chrome,
    pointer: Option<(usize, usize)>,
    hits: Vec<paint::Hit>,
    border: Option<String>,
}

enum Page {
    List,
    New { name: String },
    Rename(Line),
    KillAll,
}

impl Default for State {
    fn default() -> Self {
        Self {
            sessions: Vec::new(),
            found: Vec::new(),
            selected: None,
            finding: true,
            query: Line::new(),
            layouts: Vec::new(),
            layout_index: 0,
            current_name: None,
            page: Page::List,
            granted: false,
            notice: None,
            chrome: Chrome::default(),
            pointer: None,
            hits: Vec::new(),
            border: None,
        }
    }
}

register_plugin!(State);

impl ZellijPlugin for State {
    fn load(&mut self, _configuration: BTreeMap<String, String>) {
        request_permission(&[
            PermissionType::ReadApplicationState,
            PermissionType::ChangeApplicationState,
        ]);
        subscribe(&[
            EventType::Key,
            EventType::Mouse,
            EventType::ModeUpdate,
            EventType::PermissionRequestResult,
            EventType::SessionUpdate,
            EventType::Timer,
        ]);
        set_selectable(true);
        self.sync();
    }

    fn update(&mut self, event: Event) -> bool {
        let render = match event {
            Event::PermissionRequestResult(PermissionStatus::Granted) => {
                self.granted = true;
                if self.notice.as_deref() == Some(GRANT) {
                    self.notice = None;
                }
                self.refresh();
                set_timeout(1.0);
                true
            }
            Event::PermissionRequestResult(_) => {
                self.granted = false;
                self.notice = Some(GRANT.to_string());
                true
            }
            Event::ModeUpdate(info) => {
                let next = chrome_from(&info.style);
                if next == self.chrome {
                    false
                } else {
                    self.chrome = next;
                    true
                }
            }
            Event::SessionUpdate(..) | Event::Timer(_) => {
                if matches!(event, Event::Timer(_)) {
                    set_timeout(1.0);
                }
                self.refresh();
                true
            }
            Event::Key(key) => self.on_key(&key),
            Event::Mouse(mouse) => self.on_mouse(mouse),
            _ => false,
        };
        render
    }

    fn render(&mut self, rows: usize, cols: usize) {
        if rows == 0 || cols == 0 {
            return;
        }
        self.sync();
        self.retitle();
        let pointer = Pointer {
            y: self.pointer.map(|(y, _)| y),
            x: self.pointer.map(|(_, x)| x),
        };
        let frame = match &self.page {
            Page::List => paint_list(
                &ListView {
                    sessions: &self.sessions,
                    found: &self.found,
                    selected: self.selected,
                    finding: self.finding,
                    query: self.query.text(),
                    cursor: self.query.cursor(),
                    notice: self.list_notice(),
                },
                &self.chrome,
                rows,
                cols,
                pointer,
            ),
            Page::New { name } => self.paint_new(name, rows, cols, pointer),
            Page::Rename(line) => {
                let text = format!("Name: {}", line.text());
                let cursor_x = "Name: ".chars().count() + line.cursor();
                paint_page(
                    self.notice.as_deref(),
                    &[text],
                    Some(cursor_x),
                    &[rename_bars()],
                    &self.chrome,
                    rows,
                    cols,
                    pointer,
                )
            }
            Page::KillAll => paint_page(
                self.notice.as_deref(),
                &["Kill every other running session?".to_string()],
                None,
                &[kill_bars()],
                &self.chrome,
                rows,
                cols,
                pointer,
            ),
        };
        match frame.cursor {
            Some((x, y)) => show_cursor(Some((x, y))),
            None => show_cursor(None),
        }
        self.hits = frame.hits;
        print!("\u{1b}[?7l");
        for (y, line) in frame.lines.iter().enumerate() {
            print!("\u{1b}[{};1H{line}", y + 1);
        }
        print!("\u{1b}[?7h");
        let _ = io::stdout().flush();
    }
}

impl State {
    fn on_key(&mut self, key: &KeyWithModifier) -> bool {
        match &self.page {
            Page::List => self.list_key(key),
            Page::New { .. } => self.new_key(key),
            Page::Rename(_) => self.rename_key(key),
            Page::KillAll => self.kill_all_key(key),
        }
    }

    fn list_key(&mut self, key: &KeyWithModifier) -> bool {
        if self.finding {
            if key.has_no_modifiers() && matches!(key.bare_key, BareKey::Up) {
                self.move_selection(-1);
                return true;
            }
            if key.has_no_modifiers() && matches!(key.bare_key, BareKey::Down) {
                self.move_selection(1);
                return true;
            }
            return match self.query.handle(key) {
                Edit::Changed => {
                    self.selected = None;
                    true
                }
                Edit::Submit => {
                    self.attach_or_create();
                    true
                }
                Edit::Cancel => {
                    self.finding = false;
                    true
                }
                Edit::Complete => {
                    self.complete_query();
                    true
                }
                Edit::Ignored => false,
            };
        }
        if !key.has_no_modifiers() {
            if key.has_only_modifiers(&[KeyModifier::Ctrl]) {
                match key.bare_key {
                    BareKey::Char('a') => {
                        self.save_current();
                        return true;
                    }
                    BareKey::Char('d') => {
                        self.page = Page::KillAll;
                        return true;
                    }
                    _ => return false,
                }
            }
            return false;
        }
        match key.bare_key {
            BareKey::Up => self.move_selection(-1),
            BareKey::Down => self.move_selection(1),
            BareKey::Char('/') => {
                self.finding = true;
                true
            }
            BareKey::Enter => {
                self.attach_or_create();
                true
            }
            BareKey::Char('n') | BareKey::Char('N') => {
                self.open_new(self.query.text().to_string());
                true
            }
            BareKey::Char('r') | BareKey::Char('R') => {
                self.open_rename();
                true
            }
            BareKey::Delete => {
                self.kill_selected();
                true
            }
            BareKey::Char('x') | BareKey::Char('X') => {
                self.disconnect();
                true
            }
            BareKey::Char('a') | BareKey::Char('A') => {
                self.save_current();
                true
            }
            BareKey::Char('d') | BareKey::Char('D') => {
                self.page = Page::KillAll;
                true
            }
            BareKey::Esc => {
                close_self();
                true
            }
            _ => false,
        }
    }

    fn new_key(&mut self, key: &KeyWithModifier) -> bool {
        if !key.has_no_modifiers() {
            return false;
        }
        match key.bare_key {
            BareKey::Up => {
                self.layout_index = self.layout_index.saturating_sub(1);
                true
            }
            BareKey::Down => {
                if self.layout_index + 1 < self.layouts.len() {
                    self.layout_index += 1;
                }
                true
            }
            BareKey::Enter => {
                self.create_session();
                true
            }
            BareKey::Esc => {
                self.page = Page::List;
                self.notice = None;
                true
            }
            _ => false,
        }
    }

    fn rename_key(&mut self, key: &KeyWithModifier) -> bool {
        let edit = {
            let Page::Rename(line) = &mut self.page else {
                return false;
            };
            let edit = line.handle_form(key);
            let typed = line.text().to_string();
            (edit, typed)
        };
        match edit.0 {
            Edit::Changed | Edit::Complete | Edit::Ignored => true,
            Edit::Cancel => {
                self.page = Page::List;
                self.notice = None;
                true
            }
            Edit::Submit => {
                self.commit_rename(edit.1);
                true
            }
        }
    }

    fn kill_all_key(&mut self, key: &KeyWithModifier) -> bool {
        if !key.has_no_modifiers() {
            return false;
        }
        match key.bare_key {
            BareKey::Enter => {
                self.kill_others();
                true
            }
            BareKey::Esc => {
                self.page = Page::List;
                true
            }
            _ => false,
        }
    }

    fn on_mouse(&mut self, mouse: Mouse) -> bool {
        match mouse {
            Mouse::ScrollUp(_) => self.move_selection(-1),
            Mouse::ScrollDown(_) => self.move_selection(1),
            Mouse::Hover(line, column) => {
                self.pointer = Some((line.max(0) as usize, column));
                true
            }
            Mouse::LeftClick(line, column) => {
                let line = line.max(0) as usize;
                self.pointer = Some((line, column));
                if let Some(click) = self.hit_at(line, column) {
                    self.on_click(click);
                }
                true
            }
            _ => false,
        }
    }

    fn on_click(&mut self, click: Click) {
        match click {
            Click::Up => {
                self.move_selection(-1);
            }
            Click::Down => {
                self.move_selection(1);
            }
            Click::Complete => self.complete_query(),
            Click::Attach => self.attach_or_create(),
            Click::Find => self.finding = true,
            Click::Clear => {
                if self.query.text().is_empty() {
                    self.finding = false;
                } else {
                    self.query.clear();
                    self.selected = None;
                }
            }
            Click::Close => close_self(),
            Click::New => self.open_new(self.query.text().to_string()),
            Click::Rename => self.open_rename(),
            Click::Kill => self.kill_selected(),
            Click::Disconnect => self.disconnect(),
            Click::Save => self.save_current(),
            Click::KillAll => self.page = Page::KillAll,
            Click::Confirm => {
                let rename = match &self.page {
                    Page::Rename(line) => Some(line.text().to_string()),
                    _ => None,
                };
                if let Some(name) = rename {
                    self.commit_rename(name);
                } else if matches!(self.page, Page::New { .. }) {
                    self.create_session();
                } else if matches!(self.page, Page::KillAll) {
                    self.kill_others();
                }
            }
            Click::Cancel => {
                self.page = Page::List;
                self.notice = None;
            }
            Click::Layout(index) => {
                if self.layout_index == index {
                    self.create_session();
                } else if index < self.layouts.len() {
                    self.layout_index = index;
                }
            }
            Click::Row(index) => {
                if self.selected == Some(index) {
                    self.attach_or_create();
                } else {
                    self.selected = Some(index);
                }
            }
        }
    }

    fn move_selection(&mut self, delta: isize) -> bool {
        let len = self.found.len();
        if len == 0 {
            return true;
        }
        let current = self.selected.unwrap_or(0);
        self.selected = Some(if delta < 0 {
            if current == 0 {
                len - 1
            } else {
                current - 1
            }
        } else if current + 1 >= len {
            0
        } else {
            current + delta as usize
        });
        true
    }

    fn complete_query(&mut self) {
        let index = self.selected.unwrap_or(0);
        if let Some(name) = self
            .found
            .get(index)
            .map(|found| self.sessions[found.index].name.clone())
        {
            self.query.set(name);
            self.selected = None;
        }
    }

    fn attach_or_create(&mut self) {
        if !self.require_change() {
            return;
        }
        if let Some(session) = self.selected_session().cloned() {
            self.open_session(&session);
            return;
        }
        let typed = self.query.text().to_string();
        if typed.is_empty() {
            return;
        }
        if let Some(message) = name_error(&typed) {
            self.notice = Some(message);
            return;
        }
        if let Some(session) = self
            .sessions
            .iter()
            .find(|session| session.name == typed)
            .cloned()
        {
            self.open_session(&session);
            return;
        }
        self.open_new(typed);
    }

    fn open_session(&mut self, session: &Session) {
        if session.current {
            self.notice = Some("Already in this session.".to_string());
            return;
        }
        match session.kind {
            Kind::Running => switch_session_with_focus(&session.name, None, None),
            Kind::Exited => switch_session(Some(&session.name)),
        }
        close_self();
    }

    fn open_new(&mut self, name: String) {
        let name = if name.is_empty() && self.granted {
            generate_random_name()
        } else {
            name
        };
        if let Some(message) = name_error(&name) {
            self.notice = Some(message);
            return;
        }
        self.layout_index = 0;
        self.page = Page::New { name };
        self.notice = None;
    }

    fn create_session(&mut self) {
        if !self.require_change() {
            return;
        }
        let Page::New { name } = &self.page else {
            return;
        };
        let name = name.clone();
        let layout = self.layouts.get(self.layout_index).cloned();
        match layout {
            Some(layout) => switch_session_with_layout(Some(&name), layout, None),
            None => switch_session(Some(&name)),
        }
        close_self();
    }

    fn open_rename(&mut self) {
        let Some(name) = self.current_name.clone() else {
            self.notice = Some("There is no current session to rename.".to_string());
            return;
        };
        let mut line = Line::new();
        line.set(name);
        self.page = Page::Rename(line);
        self.notice = None;
    }

    fn commit_rename(&mut self, name: String) {
        if !self.require_change() {
            return;
        }
        if let Some(message) = name_error(&name) {
            self.notice = Some(message);
            return;
        }
        if self.sessions.iter().any(|session| session.name == name) {
            self.notice = Some("A session by this name already exists.".to_string());
            return;
        }
        rename_session(&name);
        self.current_name = Some(name);
        self.page = Page::List;
        self.notice = None;
        self.refresh();
    }

    fn kill_selected(&mut self) {
        if !self.require_change() {
            return;
        }
        let Some(session) = self.selected_session().cloned() else {
            self.notice = Some("Select a session first.".to_string());
            return;
        };
        let result = if session.can_kill() {
            kill_sessions(&[session.name.as_str()])
        } else if session.can_delete() {
            delete_dead_session(&session.name)
        } else {
            self.notice = Some("Quit to end the current session.".to_string());
            return;
        };
        self.notice = result.err();
        self.refresh();
    }

    fn kill_others(&mut self) {
        if !self.require_change() {
            return;
        }
        let names: Vec<String> = self
            .sessions
            .iter()
            .filter(|session| session.can_kill())
            .map(|session| session.name.clone())
            .collect();
        if names.is_empty() {
            self.notice = Some("No other sessions to kill.".to_string());
            self.page = Page::List;
            return;
        }
        let refs: Vec<&str> = names.iter().map(String::as_str).collect();
        self.notice = kill_sessions(&refs).err();
        self.page = Page::List;
        self.refresh();
    }

    fn disconnect(&mut self) {
        if !self.require_change() {
            return;
        }
        disconnect_other_clients();
    }

    fn save_current(&mut self) {
        if !self.require_change() {
            return;
        }
        self.notice = save_session().err();
    }

    fn refresh(&mut self) {
        if !self.granted {
            return;
        }
        let Ok(snapshot) = get_session_list() else {
            return;
        };
        if let Some(current) = snapshot
            .live_sessions
            .iter()
            .find(|session| session.is_current_session)
        {
            self.current_name = Some(current.name.clone());
            self.layouts = current.available_layouts.clone();
        }
        let mut sessions: Vec<Session> = snapshot
            .live_sessions
            .into_iter()
            .map(|session| Session {
                name: session.name,
                kind: Kind::Running,
                current: session.is_current_session,
                clients: session.connected_clients,
                tabs: session.tabs.len(),
                age: session.creation_time,
            })
            .collect();
        sessions.extend(
            snapshot
                .resurrectable_sessions
                .into_iter()
                .map(|(name, age)| Session {
                    name,
                    kind: Kind::Exited,
                    current: false,
                    clients: 0,
                    tabs: 0,
                    age,
                }),
        );
        sort_sessions(&mut sessions);
        self.sessions = sessions;
        self.sync();
    }

    fn sync(&mut self) {
        self.found = filter(&self.sessions, self.query.text());
        if let Some(selected) = self.selected {
            if selected >= self.found.len() {
                self.selected = None;
            }
        }
    }

    fn selected_session(&self) -> Option<&Session> {
        self.selected
            .and_then(|index| self.found.get(index))
            .and_then(|found| self.sessions.get(found.index))
    }

    fn list_notice(&self) -> Option<&str> {
        if let Some(notice) = self.notice.as_deref() {
            return Some(notice);
        }
        if self.granted {
            None
        } else {
            Some(GRANT)
        }
    }

    fn require_change(&mut self) -> bool {
        if self.granted {
            true
        } else {
            self.notice = Some(GRANT.to_string());
            false
        }
    }

    fn border_title(&self) -> String {
        match &self.page {
            Page::List if self.finding => "Sessions > Find".to_string(),
            Page::List => "Sessions".to_string(),
            Page::New { .. } => "Sessions > New".to_string(),
            Page::Rename(_) => "Sessions > Rename".to_string(),
            Page::KillAll => "Sessions > Kill All".to_string(),
        }
    }

    fn retitle(&mut self) {
        if !self.granted {
            return;
        }
        let title = self.border_title();
        if self.border.as_deref() == Some(title.as_str()) {
            return;
        }
        rename_plugin_pane(get_plugin_ids().plugin_id, &title);
        self.border = Some(title);
    }

    fn hit_at(&self, y: usize, x: usize) -> Option<Click> {
        self.hits.iter().find_map(|hit| {
            if hit.y == y && x >= hit.x0 && x < hit.x1 {
                Some(hit.click)
            } else {
                None
            }
        })
    }

    fn paint_new(&self, name: &str, rows: usize, cols: usize, pointer: Pointer) -> paint::Frame {
        let mut body = vec![format!("Name: {name}"), String::new()];
        if self.layouts.is_empty() {
            body.push(
                "No layouts are available. ENTER starts the session with the default layout."
                    .to_string(),
            );
        } else {
            for (index, layout) in self.layouts.iter().enumerate() {
                let mark = if index == self.layout_index {
                    "\u{1}"
                } else {
                    "\u{2}"
                };
                body.push(format!("{mark}{}", layout_label(layout)));
            }
        }
        paint_page(
            self.notice.as_deref(),
            &body,
            None,
            &[new_bars()],
            &self.chrome,
            rows,
            cols,
            pointer,
        )
    }
}

fn name_error(name: &str) -> Option<String> {
    if name.is_empty() {
        Some("The session name cannot be empty.".to_string())
    } else if name.len() >= 108 {
        Some("The session name must be shorter than 108 bytes.".to_string())
    } else if name.contains('/') {
        Some("The session name cannot contain '/'.".to_string())
    } else {
        None
    }
}

trait FormLine {
    fn handle_form(&mut self, key: &KeyWithModifier) -> Edit;
}

impl FormLine for Line {
    fn handle_form(&mut self, key: &KeyWithModifier) -> Edit {
        if key.has_no_modifiers() && matches!(key.bare_key, BareKey::Esc) {
            return Edit::Cancel;
        }
        if key.has_only_modifiers(&[KeyModifier::Ctrl])
            && matches!(key.bare_key, BareKey::Char('c'))
        {
            return Edit::Cancel;
        }
        self.handle(key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::line::key_with;

    #[test]
    fn the_pane_opens_in_find() {
        let state = State::default();
        assert!(state.finding);
        assert_eq!(state.border_title(), "Sessions > Find");
    }

    #[test]
    fn leaving_find_returns_the_title_to_the_plugin_name() {
        let mut state = State::default();
        assert!(state.list_key(&key_with(BareKey::Esc, &[])));
        assert!(!state.finding);
        assert_eq!(state.border_title(), "Sessions");
    }
}
