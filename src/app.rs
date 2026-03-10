use std::collections::HashMap;
use std::sync::mpsc;

use crate::scanner::{self, PortInfo};

#[derive(Clone, Copy, PartialEq)]
pub enum View {
    List,
    Actions,
    Confirm,
}

#[derive(Clone, Copy, PartialEq)]
pub enum Action {
    Kill,
    ForceKill,
    Refresh,
    Back,
}

pub const ACTIONS: &[Action] = &[
    Action::Kill,
    Action::ForceKill,
    Action::Refresh,
    Action::Back,
];

impl Action {
    pub fn label(&self) -> &'static str {
        match self {
            Action::Kill      => "Kill (SIGTERM)",
            Action::ForceKill => "Force Kill (SIGKILL)",
            Action::Refresh   => "Refresh",
            Action::Back      => "Back",
        }
    }
}

// ── Dynamic filters ─────────────────────────────────────────────────────────

pub struct FilterTab {
    pub label: String,
    pub lang: Option<String>,
    pub is_dev: bool,
}

pub struct App {
    /// All scanned entries (unfiltered)
    all_entries: Vec<PortInfo>,
    /// Filtered view shown in the list
    pub entries: Vec<PortInfo>,
    pub selected: usize,
    pub scroll_offset: usize,
    pub filters: Vec<FilterTab>,
    pub filter_idx: usize,
    pub running: bool,
    pub view: View,
    pub action_idx: usize,
    pub confirm_force: bool,
    pub toast: Option<Toast>,
    pub table_height: usize,
    /// Background scan channel
    scan_rx: Option<mpsc::Receiver<Vec<PortInfo>>>,
    pub scanning: bool,
    /// Search
    pub searching: bool,
    pub search_query: String,
}

pub struct Toast {
    pub message: String,
    pub is_error: bool,
    pub created: std::time::Instant,
}

impl App {
    pub fn new() -> Self {
        let mut app = Self {
            all_entries: Vec::new(),
            entries: Vec::new(),
            selected: 0,
            scroll_offset: 0,
            filters: vec![
                FilterTab { label: "Dev".into(), lang: None, is_dev: true },
                FilterTab { label: "All".into(), lang: None, is_dev: false },
            ],
            filter_idx: 0,
            running: true,
            view: View::List,
            action_idx: 0,
            confirm_force: false,
            toast: None,
            table_height: 20,
            scan_rx: None,
            scanning: false,
            searching: false,
            search_query: String::new(),
        };
        app.start_scan();
        app
    }

    // ── Background scanning ─────────────────────────────────────────

    fn start_scan(&mut self) {
        if self.scanning {
            return;
        }
        self.scanning = true;
        let (tx, rx) = mpsc::channel();
        self.scan_rx = Some(rx);
        std::thread::spawn(move || {
            // Scan ALL listening ports; filtering happens client-side
            let results = scanner::scan_all_ports();
            let _ = tx.send(results);
        });
    }

    /// Non-blocking check for background scan results. Call from main loop.
    pub fn check_scan(&mut self) {
        let rx = match self.scan_rx.as_ref() {
            Some(rx) => rx,
            None => return,
        };

        match rx.try_recv() {
            Ok(results) => {
                let prev_port = self.selected_entry().map(|e| e.port);
                self.all_entries = results;
                self.rebuild_filters();
                self.apply_filter();

                // Try to restore selection
                if let Some(port) = prev_port {
                    if let Some(idx) = self.entries.iter().position(|e| e.port == port) {
                        self.selected = idx;
                        self.ensure_visible();
                        self.scanning = false;
                        self.scan_rx = None;
                        return;
                    }
                }
                if self.selected >= self.entries.len() && !self.entries.is_empty() {
                    self.selected = self.entries.len() - 1;
                }
                self.ensure_visible();
                self.scanning = false;
                self.scan_rx = None;
            }
            Err(mpsc::TryRecvError::Empty) => {} // still scanning
            Err(mpsc::TryRecvError::Disconnected) => {
                self.scanning = false;
                self.scan_rx = None;
            }
        }
    }

    // ── Filters ─────────────────────────────────────────────────────

    fn rebuild_filters(&mut self) {
        let old_label = self.filters.get(self.filter_idx).map(|f| f.label.clone());

        let mut filters = vec![
            FilterTab { label: "Dev".into(), lang: None, is_dev: true },
            FilterTab { label: "All".into(), lang: None, is_dev: false },
        ];

        // Collect unique languages from scanned ports (skip Unknown)
        let mut langs: Vec<String> = self.all_entries.iter()
            .map(|e| e.language.clone())
            .filter(|l| l != "Unknown")
            .collect();
        langs.sort();
        langs.dedup();

        for lang in langs {
            filters.push(FilterTab {
                label: lang.clone(),
                lang: Some(lang),
                is_dev: false,
            });
        }

        // Keep same filter selected if still exists
        if let Some(old) = old_label {
            if let Some(idx) = filters.iter().position(|f| f.label == old) {
                self.filter_idx = idx;
            } else {
                self.filter_idx = 0;
            }
        }

        self.filters = filters;
    }

    fn apply_filter(&mut self) {
        if self.filters.is_empty() {
            self.entries = self.all_entries.clone();
            return;
        }

        let filter = &self.filters[self.filter_idx];

        if filter.is_dev {
            let mut entries: Vec<PortInfo> = self.all_entries.iter()
                .filter(|e| e.framework != "Unknown")
                .cloned()
                .collect();
            let mut seen = HashMap::new();
            entries.retain(|r| seen.insert(r.pid, true).is_none());
            self.entries = entries;
        } else if let Some(ref lang) = filter.lang {
            self.entries = self.all_entries.iter()
                .filter(|e| &e.language == lang)
                .cloned()
                .collect();
        } else {
            self.entries = self.all_entries.clone();
        }

        // Apply search query on top of filter
        if !self.search_query.is_empty() {
            let q = self.search_query.to_lowercase();
            self.entries.retain(|e| {
                e.port.to_string().contains(&q)
                    || e.pid.to_string().contains(&q)
                    || e.project.to_lowercase().contains(&q)
                    || e.framework.to_lowercase().contains(&q)
                    || e.language.to_lowercase().contains(&q)
                    || e.start_cmd.to_lowercase().contains(&q)
                    || e.name.to_lowercase().contains(&q)
                    || e.user.to_lowercase().contains(&q)
            });
        }
    }

    pub fn cycle_filter_next(&mut self) {
        if self.filters.is_empty() {
            return;
        }
        self.filter_idx = (self.filter_idx + 1) % self.filters.len();
        self.apply_filter();
        self.selected = 0;
        self.scroll_offset = 0;
        let msg = format!("Filter: {}", self.filters[self.filter_idx].label);
        self.set_toast(msg, false);
    }

    pub fn cycle_filter_prev(&mut self) {
        if self.filters.is_empty() {
            return;
        }
        if self.filter_idx == 0 {
            self.filter_idx = self.filters.len() - 1;
        } else {
            self.filter_idx -= 1;
        }
        self.apply_filter();
        self.selected = 0;
        self.scroll_offset = 0;
        let msg = format!("Filter: {}", self.filters[self.filter_idx].label);
        self.set_toast(msg, false);
    }

    // ── Refresh ─────────────────────────────────────────────────────

    pub fn refresh(&mut self) {
        self.start_scan();
    }

    // ── Navigation ──────────────────────────────────────────────────

    pub fn selected_entry(&self) -> Option<&PortInfo> {
        self.entries.get(self.selected)
    }

    pub fn select_next(&mut self) {
        if !self.entries.is_empty() && self.selected < self.entries.len() - 1 {
            self.selected += 1;
            self.ensure_visible();
        }
    }

    pub fn select_prev(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
            self.ensure_visible();
        }
    }

    pub fn select_first(&mut self) {
        self.selected = 0;
        self.scroll_offset = 0;
    }

    pub fn select_last(&mut self) {
        if !self.entries.is_empty() {
            self.selected = self.entries.len() - 1;
            self.ensure_visible();
        }
    }

    fn ensure_visible(&mut self) {
        let visible = self.table_height.saturating_sub(3);
        if visible == 0 { return; }
        if self.selected < self.scroll_offset {
            self.scroll_offset = self.selected;
        } else if self.selected >= self.scroll_offset + visible {
            self.scroll_offset = self.selected - visible + 1;
        }
    }

    pub fn visible_range(&self) -> std::ops::Range<usize> {
        let visible = self.table_height.saturating_sub(3);
        let start = self.scroll_offset;
        let end = (start + visible).min(self.entries.len());
        start..end
    }

    // ── Actions ─────────────────────────────────────────────────────

    pub fn open_actions(&mut self) {
        if !self.entries.is_empty() {
            self.action_idx = 0;
            self.view = View::Actions;
        }
    }

    pub fn action_next(&mut self) {
        if self.action_idx < ACTIONS.len() - 1 {
            self.action_idx += 1;
        }
    }

    pub fn action_prev(&mut self) {
        if self.action_idx > 0 {
            self.action_idx -= 1;
        }
    }

    pub fn execute_action(&mut self) {
        let action = ACTIONS[self.action_idx];
        match action {
            Action::Kill => {
                self.confirm_force = false;
                self.view = View::Confirm;
            }
            Action::ForceKill => {
                self.confirm_force = true;
                self.view = View::Confirm;
            }
            Action::Refresh => {
                self.refresh();
                self.set_toast("Refreshed".into(), false);
                self.view = View::List;
            }
            Action::Back => {
                self.view = View::List;
            }
        }
    }

    pub fn do_kill(&mut self) {
        if let Some(entry) = self.selected_entry().cloned() {
            let (ok, msg) = scanner::kill_port(entry.port, self.confirm_force);
            self.set_toast(msg, !ok);
            self.refresh();
        }
        self.view = View::List;
    }

    pub fn go_back(&mut self) {
        match self.view {
            View::Actions => self.view = View::List,
            View::Confirm => self.view = View::Actions,
            View::List => {}
        }
    }

    // ── Search ───────────────────────────────────────────────────

    pub fn search_open(&mut self) {
        self.searching = true;
        self.search_query.clear();
    }

    pub fn search_push(&mut self, ch: char) {
        self.search_query.push(ch);
        self.apply_filter();
        self.selected = 0;
        self.scroll_offset = 0;
    }

    pub fn search_pop(&mut self) {
        self.search_query.pop();
        self.apply_filter();
        self.selected = 0;
        self.scroll_offset = 0;
    }

    pub fn search_confirm(&mut self) {
        self.searching = false;
        // Keep the query active (entries stay filtered)
    }

    pub fn search_cancel(&mut self) {
        self.searching = false;
        self.search_query.clear();
        self.apply_filter();
        self.selected = 0;
        self.scroll_offset = 0;
    }

    pub fn set_toast(&mut self, message: String, is_error: bool) {
        self.toast = Some(Toast {
            message,
            is_error,
            created: std::time::Instant::now(),
        });
    }

    pub fn tick_toast(&mut self) {
        if let Some(ref t) = self.toast {
            if t.created.elapsed().as_secs() >= 3 {
                self.toast = None;
            }
        }
    }
}
