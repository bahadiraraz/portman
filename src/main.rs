mod app;
mod event;
mod scanner;
mod ui;

use std::io;
use std::time::{Duration, Instant};

use clap::{Parser, Subcommand};
use crossterm::{
    event::{
        DisableBracketedPaste, EnableBracketedPaste,
        KeyboardEnhancementFlags, PopKeyboardEnhancementFlags,
        PushKeyboardEnhancementFlags,
    },
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::prelude::*;

#[derive(Parser)]
#[command(name = "portman", about = "See what's running on your dev ports")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// List all listening dev ports
    List {
        #[arg(short, long)]
        all: bool,
        #[arg(short, long)]
        json: bool,
    },
    /// Show ALL listening TCP ports with process info (like netstat)
    Ps {
        #[arg(short, long)]
        json: bool,
    },
    /// Show detailed info for a specific port
    Info { port: u16 },
    /// Kill the process on a specific port
    Kill {
        port: u16,
        #[arg(short, long)]
        force: bool,
    },
}

fn main() -> io::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Some(Commands::List { all, json }) => cmd_list(all, json),
        Some(Commands::Ps { json }) => cmd_ps(json),
        Some(Commands::Info { port }) => cmd_info(port),
        Some(Commands::Kill { port, force }) => cmd_kill(port, force),
        None => run_tui(),
    }
}

// ── CLI ─────────────────────────────────────────────────────────────────────

fn cmd_list(all: bool, json_out: bool) -> io::Result<()> {
    let entries = scanner::scan_ports(!all);
    if json_out {
        println!("{}", serde_json::to_string_pretty(&entries).unwrap());
        return Ok(());
    }
    if entries.is_empty() {
        println!("  No listening dev ports found.");
        return Ok(());
    }
    println!();
    println!("  {:<6} {:<7} {:<20} {:<14} {:<8}", "PORT", "PID", "PROJECT", "FRAMEWORK", "LANG");
    println!("  {}", "─".repeat(60));
    for e in &entries {
        println!("  {:<6} {:<7} {:<20} {:<14} {:<8}", e.port, e.pid, trunc(&e.project, 20), e.framework, e.language);
    }
    println!("\n  {} port(s)\n", entries.len());
    Ok(())
}

fn cmd_ps(json_out: bool) -> io::Result<()> {
    let entries = scanner::scan_all_ports();
    if json_out {
        println!("{}", serde_json::to_string_pretty(&entries).unwrap());
        return Ok(());
    }
    if entries.is_empty() {
        println!("  No listening TCP ports found.");
        return Ok(());
    }
    println!();
    println!(
        "  {:<7} {:<7} {:<10} {:<18} {:<14} COMMAND",
        "PORT", "PID", "USER", "PROCESS", "FRAMEWORK"
    );
    println!("  {}", "─".repeat(78));
    for e in &entries {
        let pname = std::path::Path::new(&e.name)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| e.name.clone());
        println!(
            "  {:<7} {:<7} {:<10} {:<18} {:<14} {}",
            e.port,
            e.pid,
            trunc(&e.user, 10),
            trunc(&pname, 18),
            e.framework,
            trunc(&e.start_cmd, 40)
        );
    }
    println!("\n  {} port(s)\n", entries.len());
    Ok(())
}

fn cmd_info(port: u16) -> io::Result<()> {
    match scanner::scan_port(port) {
        Some(e) => {
            println!("\n  Port:       {}\n  PID:        {}\n  Process:    {}\n  Project:    {}\n  Framework:  {}\n  Language:   {}\n  User:       {}\n  Command:    {}\n  CWD:        {}\n",
                e.port, e.pid, e.name, e.project, e.framework, e.language, e.user, e.start_cmd, e.cwd.as_deref().unwrap_or("?"));
        }
        None => eprintln!("  No process found on port {port}"),
    }
    Ok(())
}

fn cmd_kill(port: u16, force: bool) -> io::Result<()> {
    let (ok, msg) = scanner::kill_port(port, force);
    if ok { println!("  {msg}"); } else { eprintln!("  {msg}"); }
    Ok(())
}

// ── TUI ─────────────────────────────────────────────────────────────────────

fn run_tui() -> io::Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();

    // Codex-style keyboard enhancement: disambiguate escape codes
    let has_enhanced_kb = execute!(
        stdout,
        PushKeyboardEnhancementFlags(
            KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
                | KeyboardEnhancementFlags::REPORT_EVENT_TYPES
        )
    ).is_ok();

    execute!(stdout, EnterAlternateScreen, EnableBracketedPaste)?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    let mut app = app::App::new();
    let mut last_refresh = Instant::now();
    let mut last_input = Instant::now();
    let refresh_interval = Duration::from_secs(3);

    loop {
        // 1) Draw
        terminal.draw(|f| ui::render(f, &mut app))?;

        // 2) Process ALL pending input events (non-blocking drain)
        let had_input = event::drain_events(&mut app);
        if had_input {
            last_input = Instant::now();
        }

        if !app.running {
            break;
        }

        // 3) Check for background scan results (non-blocking)
        app.check_scan();

        // 4) Auto-refresh only if idle and not already scanning
        if last_refresh.elapsed() >= refresh_interval
            && last_input.elapsed() >= Duration::from_millis(500)
            && !app.scanning
        {
            app.refresh();
            last_refresh = Instant::now();
        }

        app.tick_toast();
    }

    // Restore terminal
    disable_raw_mode()?;
    if has_enhanced_kb {
        let _ = execute!(terminal.backend_mut(), PopKeyboardEnhancementFlags);
    }
    execute!(terminal.backend_mut(), DisableBracketedPaste, LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    Ok(())
}

fn trunc(s: &str, max: usize) -> String {
    if s.chars().count() <= max { s.to_string() } else {
        let t: String = s.chars().take(max - 1).collect();
        format!("{t}…")
    }
}
