use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

use crate::app::{App, View, ACTIONS};

// Codex style: ANSI colors only, terminal background untouched
const CYAN: Color = Color::Cyan;
const DIM: Color = Color::DarkGray;
const GREEN: Color = Color::Green;
const RED: Color = Color::Red;
const BOLD: Modifier = Modifier::BOLD;

fn lang_color(l: &str) -> Color {
    match l {
        "JS/TS" | "JS" | "TS" => Color::Yellow,
        "Python"               => Color::Blue,
        "Java" | "Kotlin" | "Scala" => Color::Red,
        "Go"                   => Color::Cyan,
        "Rust"                 => Color::LightRed,
        "Ruby" | "Elixir"      => Color::Magenta,
        "PHP"                  => Color::LightMagenta,
        _ => Color::Reset,
    }
}

// ── Main ────────────────────────────────────────────────────────────────────

pub fn render(f: &mut Frame, app: &mut App) {
    let area = f.area();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // topbar + filter tabs
            Constraint::Min(1),   // main
            Constraint::Length(1), // footer
        ])
        .split(area);

    draw_topbar(f, app, chunks[0]);
    draw_main(f, app, chunks[1]);
    draw_footer(f, app, chunks[2]);

    if let Some(ref t) = app.toast {
        draw_toast(f, t, area);
    }

    if app.view == View::Confirm {
        draw_confirm(f, app, area);
    }
}

// ── Topbar ──────────────────────────────────────────────────────────────────

fn draw_topbar(f: &mut Frame, app: &App, area: Rect) {
    let dot = if app.entries.is_empty() { DIM } else { GREEN };

    // Line 1: brand + port count + scanning indicator
    let mut line1_spans = vec![
        s(" ⚓ ", CYAN, BOLD),
        s("portman", CYAN, BOLD),
        s("  │  ", DIM, Modifier::empty()),
        s("● ", dot, Modifier::empty()),
        s(&app.entries.len().to_string(), Color::Reset, BOLD),
        s(" port(s)", DIM, Modifier::empty()),
    ];
    if app.scanning {
        line1_spans.push(s("  │  ", DIM, Modifier::empty()));
        line1_spans.push(s("scanning…", DIM, Modifier::ITALIC));
    }
    let line1 = Line::from(line1_spans);

    // Line 2: filter tabs (dynamic — only shows languages that exist)
    let mut tabs: Vec<Span<'static>> = vec![s(" ", Color::Reset, Modifier::empty())];
    for (i, filter) in app.filters.iter().enumerate() {
        let active = i == app.filter_idx;
        if active {
            tabs.push(s(" ", Color::Reset, Modifier::empty()));
            tabs.push(s(&filter.label, CYAN, BOLD | Modifier::UNDERLINED));
            tabs.push(s(" ", Color::Reset, Modifier::empty()));
        } else {
            tabs.push(s(" ", Color::Reset, Modifier::empty()));
            tabs.push(s(&filter.label, DIM, Modifier::empty()));
            tabs.push(s(" ", Color::Reset, Modifier::empty()));
        }
    }
    let line2 = Line::from(tabs);

    let lines = vec![line1, Line::raw(""), line2];
    f.render_widget(Paragraph::new(lines), area);
}

// ── Main ────────────────────────────────────────────────────────────────────

fn draw_main(f: &mut Frame, app: &mut App, area: Rect) {
    if app.entries.is_empty() {
        draw_empty(f, area);
        return;
    }

    match app.view {
        View::List => {
            draw_list(f, app, area);
        }
        View::Actions | View::Confirm => {
            let cols = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
                .split(area);
            draw_list(f, app, cols[0]);
            draw_actions(f, app, cols[1]);
        }
    }
}

fn draw_empty(f: &mut Frame, area: Rect) {
    // Anchor ASCII art — the portman icon
    let anchor_art: &[&str] = &[
        r"          ╷          ",
        r"        ╶─●─╴        ",
        r"          │          ",
        r"     ╶────┼────╴     ",
        r"          │          ",
        r"          │          ",
        r"          │          ",
        r"      ╭───┴───╮     ",
        r"    ╭─╯       ╰─╮   ",
        r"    │           │   ",
        r"    ╰─╮       ╭─╯   ",
        r"      ╰───────╯     ",
    ];

    let total_h = anchor_art.len() + 7; // art + spacing + text lines
    let top_pad = (area.height as usize).saturating_sub(total_h) / 2;
    let mut lines: Vec<Line> = Vec::new();
    for _ in 0..top_pad {
        lines.push(Line::raw(""));
    }

    // Center anchor art
    let art_w = 21;
    let pad = (area.width as usize).saturating_sub(art_w) / 2;
    let pad_str: String = " ".repeat(pad);

    for row in anchor_art {
        lines.push(Line::from(vec![
            s(&pad_str, Color::Reset, Modifier::empty()),
            s(row, CYAN, Modifier::empty()),
        ]));
    }

    lines.push(Line::raw(""));

    // Brand name
    let name = "p o r t m a n";
    let name_pad = " ".repeat((area.width as usize).saturating_sub(name.len()) / 2);
    lines.push(Line::from(vec![
        s(&name_pad, Color::Reset, Modifier::empty()),
        s(name, CYAN, BOLD),
    ]));

    let tag = "dev port manager";
    let tag_pad = " ".repeat((area.width as usize).saturating_sub(tag.len()) / 2);
    lines.push(Line::from(vec![
        s(&tag_pad, Color::Reset, Modifier::empty()),
        s(tag, DIM, Modifier::empty()),
    ]));

    lines.push(Line::raw(""));
    lines.push(Line::raw(""));

    let msg = "No dev servers running.";
    let msg_pad = " ".repeat((area.width as usize).saturating_sub(msg.len()) / 2);
    lines.push(Line::from(vec![
        s(&msg_pad, Color::Reset, Modifier::empty()),
        s(msg, DIM, Modifier::empty()),
    ]));

    // Hint line
    let hint_parts = vec![
        s("Tab", CYAN, BOLD),
        s(" change filter   ", DIM, Modifier::empty()),
        s("q", CYAN, BOLD),
        s(" quit", DIM, Modifier::empty()),
    ];
    let hint_w = 28;
    let hint_pad = " ".repeat((area.width as usize).saturating_sub(hint_w) / 2);
    let mut hint = vec![s(&hint_pad, Color::Reset, Modifier::empty())];
    hint.extend(hint_parts);
    lines.push(Line::from(hint));

    f.render_widget(Paragraph::new(lines), area);
}

// ── List (direct Line rendering, no Table widget) ───────────────────────────

fn draw_list(f: &mut Frame, app: &mut App, area: Rect) {
    let h = area.height as usize;
    app.table_height = h;

    // Header
    let header = Line::from(vec![
        s("  ", Color::Reset, Modifier::empty()),
        s(&fmt_col("PORT", 7), DIM, BOLD),
        s(&fmt_col("PID", 8), DIM, BOLD),
        s(&fmt_col("PROJECT", 22), DIM, BOLD),
        s(&fmt_col("FRAMEWORK", 14), DIM, BOLD),
        s(&fmt_col("LANG", 8), DIM, BOLD),
    ]);

    let separator = Line::from(vec![
        s(&format!("  {}", "─".repeat(area.width.saturating_sub(3) as usize)), DIM, Modifier::empty()),
    ]);

    let range = app.visible_range();
    let mut lines: Vec<Line> = vec![header, separator];

    for (vi, e) in app.entries[range.clone()].iter().enumerate() {
        let idx = range.start + vi;
        let sel = idx == app.selected;

        let arrow = if sel { "▸ " } else { "  " };
        let fg = if sel { CYAN } else { Color::Reset };
        let m = if sel { BOLD } else { Modifier::empty() };
        let lc = if sel { CYAN } else { lang_color(&e.language) };
        let fc = if sel { CYAN } else { GREEN };

        lines.push(Line::from(vec![
            s(arrow, fg, m),
            s(&fmt_col(&e.port.to_string(), 7), fg, m),
            s(&fmt_col(&e.pid.to_string(), 8), fg, Modifier::empty()),
            s(&fmt_col(&trunc(&e.project, 20), 22), fg, m),
            s(&fmt_col(&e.framework, 14), fc, Modifier::empty()),
            s(&fmt_col(&e.language, 8), lc, Modifier::empty()),
        ]));
    }

    // Scroll indicator
    let total = app.entries.len();
    let visible = h.saturating_sub(2);
    if total > visible {
        lines.push(Line::raw(""));
        let info = format!("  [{}/{}]", app.selected + 1, total);
        lines.push(Line::from(vec![s(&info, DIM, Modifier::empty())]));
    }

    f.render_widget(Paragraph::new(lines), area);
}

// ── Actions panel ──────────────────────────────────────────────────────────

fn draw_actions(f: &mut Frame, app: &App, area: Rect) {
    // Full bordered box around the actions panel
    let border_color = if app.view == View::Actions { CYAN } else { DIM };
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color))
        .title(Span::styled(" Detail ", Style::default().fg(CYAN).add_modifier(BOLD)));
    let inner = block.inner(area);
    f.render_widget(block, inner.union(area)); // render block on full area

    let entry = match app.selected_entry() {
        Some(e) => e,
        None => return,
    };

    let lc = lang_color(&entry.language);
    let cmd_max = (inner.width as usize).saturating_sub(15);

    let mut lines: Vec<Line> = vec![
        Line::from(vec![
            s(" ", Color::Reset, Modifier::empty()),
            s(&entry.framework, CYAN, BOLD),
            s("  ", Color::Reset, Modifier::empty()),
            s(&entry.language, lc, Modifier::empty()),
        ]),
        Line::raw(""),
        kv("PORT", &entry.port.to_string(), CYAN),
        kv("PID", &entry.pid.to_string(), Color::Reset),
        kv("PROJECT", &entry.project, Color::Reset),
        kv("USER", &entry.user, Color::Reset),
        kv("COMMAND", &trunc(&entry.start_cmd, cmd_max), GREEN),
        Line::raw(""),
    ];

    // Render action items
    for (i, action) in ACTIONS.iter().enumerate() {
        let sel = i == app.action_idx && app.view == View::Actions;
        let arrow = if sel { " ▸ " } else { "   " };
        let fg = if sel { CYAN } else { Color::Reset };
        let m = if sel { BOLD } else { Modifier::empty() };

        // Color kill actions red when selected
        let ac = match (ACTIONS[i], sel) {
            (crate::app::Action::Kill, true) | (crate::app::Action::ForceKill, true) => RED,
            _ => fg,
        };

        lines.push(Line::from(vec![
            s(arrow, ac, m),
            s(action.label(), ac, m),
        ]));
    }

    f.render_widget(Paragraph::new(lines), inner);
}

// ── Footer ──────────────────────────────────────────────────────────────────

fn draw_footer(f: &mut Frame, app: &App, area: Rect) {
    // Search mode: show search input
    if app.searching {
        let line = Line::from(vec![
            s(" /", CYAN, BOLD),
            s(&app.search_query, Color::Reset, BOLD),
            s("▎", CYAN, Modifier::empty()), // cursor
            s("  ", Color::Reset, Modifier::empty()),
            s("Enter", DIM, Modifier::empty()),
            s(" confirm  ", DIM, Modifier::empty()),
            s("Esc", DIM, Modifier::empty()),
            s(" cancel", DIM, Modifier::empty()),
        ]);
        f.render_widget(Paragraph::new(line), area);
        return;
    }

    // Active search indicator (after confirming search)
    if !app.search_query.is_empty() && app.view == View::List {
        let line = Line::from(vec![
            s(" search: ", DIM, Modifier::empty()),
            s(&app.search_query, CYAN, BOLD),
            s("  │  ", DIM, Modifier::empty()),
            s("↑↓", CYAN, BOLD), s(" nav  ", DIM, Modifier::empty()),
            s("→", CYAN, BOLD), s(" actions  ", DIM, Modifier::empty()),
            s("/", CYAN, BOLD), s(" new search  ", DIM, Modifier::empty()),
            s("Esc", CYAN, BOLD), s(" clear", DIM, Modifier::empty()),
        ]);
        f.render_widget(Paragraph::new(line), area);
        return;
    }

    let line = match app.view {
        View::List => Line::from(vec![
            s(" ↑↓", CYAN, BOLD), s(" navigate  ", DIM, Modifier::empty()),
            s("→", CYAN, BOLD), s(" actions  ", DIM, Modifier::empty()),
            s("/", CYAN, BOLD), s(" search  ", DIM, Modifier::empty()),
            s("Tab", CYAN, BOLD), s(" filter  ", DIM, Modifier::empty()),
            s("q", CYAN, BOLD), s(" quit", DIM, Modifier::empty()),
        ]),
        View::Actions => Line::from(vec![
            s(" ↑↓", CYAN, BOLD), s(" select  ", DIM, Modifier::empty()),
            s("Enter/→", CYAN, BOLD), s(" confirm  ", DIM, Modifier::empty()),
            s("←/Esc", CYAN, BOLD), s(" back", DIM, Modifier::empty()),
        ]),
        View::Confirm => Line::from(vec![
            s(" y/Enter", GREEN, BOLD), s(" confirm   ", DIM, Modifier::empty()),
            s("n/Esc", RED, BOLD), s(" cancel", DIM, Modifier::empty()),
        ]),
    };
    f.render_widget(Paragraph::new(line), area);
}

// ── Toast ───────────────────────────────────────────────────────────────────

fn draw_toast(f: &mut Frame, toast: &crate::app::Toast, area: Rect) {
    let w = (toast.message.len() + 4).min(area.width as usize) as u16;
    let x = area.width.saturating_sub(w + 1);
    let rect = Rect::new(x, 0, w, 1);
    let c = if toast.is_error { RED } else { GREEN };
    f.render_widget(Clear, rect);
    f.render_widget(Paragraph::new(Line::from(vec![s(&format!(" {} ", toast.message), c, Modifier::empty())])), rect);
}

// ── Confirm dialog ──────────────────────────────────────────────────────────

fn draw_confirm(f: &mut Frame, app: &App, area: Rect) {
    let entry = match app.selected_entry() {
        Some(e) => e,
        None => return,
    };

    let w: u16 = 46;
    let h: u16 = 11;
    let x = area.width.saturating_sub(w) / 2;
    let y = area.height.saturating_sub(h) / 2;
    let rect = Rect::new(x, y, w, h);

    f.render_widget(Clear, rect);

    let title = if app.confirm_force { "FORCE KILL" } else { "KILL PROCESS" };

    let lines = vec![
        Line::raw(""),
        Line::from(vec![s(&format!("  {title}"), RED, BOLD)]),
        Line::raw(""),
        ckv("Port", &entry.port.to_string(), CYAN),
        ckv("PID", &entry.pid.to_string(), Color::Reset),
        ckv("Project", &entry.project, Color::Reset),
        ckv("Framework", &entry.framework, Color::Reset),
        Line::raw(""),
        Line::from(vec![
            s("  ", Color::Reset, Modifier::empty()),
            s("y", GREEN, BOLD), s("/", DIM, Modifier::empty()), s("Enter", GREEN, BOLD),
            s(" confirm   ", DIM, Modifier::empty()),
            s("n", RED, BOLD), s("/", DIM, Modifier::empty()), s("Esc", RED, BOLD),
            s(" cancel", DIM, Modifier::empty()),
        ]),
    ];

    let block = Block::default().borders(Borders::ALL).border_style(Style::default().fg(RED));
    f.render_widget(Paragraph::new(lines).block(block), rect);
}

fn kv(label: &str, value: &str, vc: Color) -> Line<'static> {
    Line::from(vec![
        s(&format!("  {:<10}", label), DIM, Modifier::empty()),
        s("  ", Color::Reset, Modifier::empty()),
        s(value, vc, Modifier::empty()),
    ])
}

fn ckv(label: &str, value: &str, vc: Color) -> Line<'static> {
    Line::from(vec![
        s(&format!("  {:<12}", label), DIM, Modifier::empty()),
        s(value, vc, Modifier::empty()),
    ])
}

// ── Helpers ─────────────────────────────────────────────────────────────────

/// Create an owned Span
fn s(text: &str, fg: Color, modifier: Modifier) -> Span<'static> {
    Span::styled(text.to_string(), Style::default().fg(fg).add_modifier(modifier))
}

fn fmt_col(text: &str, width: usize) -> String {
    format!("{:<width$}", text, width = width)
}

fn trunc(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        text.to_string()
    } else {
        let t: String = text.chars().take(max - 1).collect();
        format!("{t}…")
    }
}
