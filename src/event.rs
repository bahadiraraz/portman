use std::time::Duration;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

use crate::app::{App, View};

/// Drain all pending events. Returns true if any key was pressed.
pub fn drain_events(app: &mut App) -> bool {
    let mut had_input = false;

    if !event::poll(Duration::from_millis(50)).unwrap_or(false) {
        return false;
    }

    while let Ok(ev) = event::read() {
        if let Event::Key(key) = ev {
            if matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat) {
                had_input = true;
                if !handle_key(app, key) {
                    return true;
                }
            }
        }

        if !event::poll(Duration::ZERO).unwrap_or(false) {
            break;
        }
    }

    had_input
}

fn handle_key(app: &mut App, key: KeyEvent) -> bool {
    // Ctrl-C always quits
    if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
        app.running = false;
        return false;
    }

    // Search mode intercepts all keys
    if app.searching {
        return handle_search(app, key);
    }

    match app.view {
        View::List    => handle_list(app, key),
        View::Actions => handle_actions(app, key),
        View::Confirm => handle_confirm(app, key),
    }
}

fn handle_list(app: &mut App, key: KeyEvent) -> bool {
    match key.code {
        KeyCode::Char('q') | KeyCode::Esc => {
            // If there's an active search query, clear it first
            if !app.search_query.is_empty() {
                app.search_cancel();
                return true;
            }
            app.running = false;
            false
        }

        KeyCode::Down  | KeyCode::Char('j') => { app.select_next(); true }
        KeyCode::Up    | KeyCode::Char('k') => { app.select_prev(); true }
        KeyCode::Right | KeyCode::Enter     => { app.open_actions(); true }

        KeyCode::Char('/') => { app.search_open(); true }

        KeyCode::Tab      => { app.cycle_filter_next(); true }
        KeyCode::BackTab  => { app.cycle_filter_prev(); true }

        KeyCode::PageDown => { for _ in 0..10 { app.select_next(); } true }
        KeyCode::PageUp   => { for _ in 0..10 { app.select_prev(); } true }
        KeyCode::Home | KeyCode::Char('g') => { app.select_first(); true }
        KeyCode::End  | KeyCode::Char('G') => { app.select_last(); true }

        _ => true,
    }
}

fn handle_search(app: &mut App, key: KeyEvent) -> bool {
    match key.code {
        KeyCode::Esc => {
            app.search_cancel();
            true
        }
        KeyCode::Enter => {
            app.search_confirm();
            true
        }
        KeyCode::Backspace => {
            app.search_pop();
            true
        }
        KeyCode::Char(ch) => {
            app.search_push(ch);
            true
        }
        // Allow navigation while searching
        KeyCode::Down  => { app.select_next(); true }
        KeyCode::Up    => { app.select_prev(); true }
        _ => true,
    }
}

fn handle_actions(app: &mut App, key: KeyEvent) -> bool {
    match key.code {
        KeyCode::Down  | KeyCode::Char('j') => { app.action_next(); true }
        KeyCode::Up    | KeyCode::Char('k') => { app.action_prev(); true }
        KeyCode::Enter | KeyCode::Right     => { app.execute_action(); true }
        KeyCode::Left  | KeyCode::Esc       => { app.go_back(); true }
        _ => true,
    }
}

fn handle_confirm(app: &mut App, key: KeyEvent) -> bool {
    match key.code {
        KeyCode::Char('y') | KeyCode::Enter => { app.do_kill(); true }
        KeyCode::Char('n') | KeyCode::Esc | KeyCode::Left => { app.go_back(); true }
        _ => true,
    }
}
