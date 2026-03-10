use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct PortInfo {
    pub port: u16,
    pub pid: u32,
    pub name: String,
    pub cmdline: String,
    pub start_cmd: String,
    pub cwd: Option<String>,
    pub project: String,
    pub framework: String,
    pub language: String,
    pub user: String,
}

// ── Batch scanning: 3 subprocess calls total ────────────────────────────────

/// lsof call #1: get all listening TCP ports → {port: pid}
fn get_listening_ports() -> HashMap<u16, u32> {
    let mut map = HashMap::new();
    let Ok(output) = Command::new("lsof")
        .args(["-iTCP", "-sTCP:LISTEN", "-nP", "-F", "pn"])
        .stderr(std::process::Stdio::null())
        .output()
    else {
        return map;
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut pid: Option<u32> = None;

    for line in stdout.lines() {
        if let Some(rest) = line.strip_prefix('p') {
            pid = rest.parse().ok();
        } else if line.starts_with('n') {
            if let (Some(p), Some(colon)) = (pid, line.rfind(':')) {
                if let Ok(port) = line[colon + 1..].parse::<u16>() {
                    map.entry(port).or_insert(p);
                }
            }
        }
    }
    map
}

struct ProcInfo {
    ppid: u32,
    user: String,
    name: String,
    cmdline: String,
}

/// ps call: batch get ppid, user, comm, args for pids
/// When called with no `-p` filter, gets ALL processes (for parent chain lookup)
fn batch_ps_all() -> HashMap<u32, ProcInfo> {
    let mut map = HashMap::new();

    let Ok(output) = Command::new("ps")
        .args(["-axo", "pid=,ppid=,user=,comm=,args="])
        .output()
    else {
        return map;
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        // Format: "PID PPID USER COMM ARGS..."
        let mut s = trimmed;

        // Parse PID
        s = s.trim_start();
        let i = match s.find(char::is_whitespace) {
            Some(i) => i,
            None => continue,
        };
        let pid: u32 = match s[..i].parse() {
            Ok(p) => p,
            Err(_) => continue,
        };
        s = s[i..].trim_start();

        // Parse PPID
        let i = match s.find(char::is_whitespace) {
            Some(i) => i,
            None => continue,
        };
        let ppid: u32 = match s[..i].parse() {
            Ok(p) => p,
            Err(_) => continue,
        };
        s = s[i..].trim_start();

        // Parse USER
        let i = match s.find(char::is_whitespace) {
            Some(i) => i,
            None => continue,
        };
        let user = s[..i].to_string();
        s = s[i..].trim_start();

        // Remaining: "COMM ARGS..."
        // COMM is the first token, ARGS is everything after COMM
        let (name, cmdline) = match s.find(char::is_whitespace) {
            Some(i) => (s[..i].to_string(), s[i..].trim_start().to_string()),
            None => (s.to_string(), s.to_string()),
        };

        map.insert(pid, ProcInfo { ppid, user, name, cmdline });
    }
    map
}

/// lsof call #2: batch get cwd for ALL pids in one call
fn batch_cwd(pids: &[u32]) -> HashMap<u32, String> {
    let mut map = HashMap::new();
    if pids.is_empty() {
        return map;
    }

    let pid_list: String = pids.iter().map(|p| p.to_string()).collect::<Vec<_>>().join(",");

    let Ok(output) = Command::new("lsof")
        .args(["-a", "-p", &pid_list, "-d", "cwd", "-F", "pn"])
        .stderr(std::process::Stdio::null())
        .output()
    else {
        return map;
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut current_pid: Option<u32> = None;

    for line in stdout.lines() {
        if let Some(rest) = line.strip_prefix('p') {
            current_pid = rest.parse().ok();
        } else if let Some(path) = line.strip_prefix('n') {
            if let Some(pid) = current_pid {
                if path != "/" {
                    map.insert(pid, path.to_string());
                }
            }
        }
    }
    map
}

// ── Framework detection ─────────────────────────────────────────────────────

struct Sig(&'static str, &'static str, &'static str); // pattern, framework, language

const CMD_SIGS: &[Sig] = &[
    Sig("next", "Next.js", "JS/TS"),
    Sig("vite", "Vite", "JS/TS"),
    Sig("nuxt", "Nuxt", "JS/TS"),
    Sig("remix", "Remix", "JS/TS"),
    Sig("astro", "Astro", "JS/TS"),
    Sig("webpack-dev-server", "Webpack", "JS/TS"),
    Sig("react-scripts", "CRA", "JS/TS"),
    Sig("ng serve", "Angular", "TS"),
    Sig("angular", "Angular", "TS"),
    Sig("svelte", "SvelteKit", "JS/TS"),
    Sig("gatsby", "Gatsby", "JS/TS"),
    Sig("express", "Express", "JS/TS"),
    Sig("nest", "NestJS", "TS"),
    Sig("ts-node", "Node.js", "TS"),
    Sig("tsx", "Node.js", "TS"),
    Sig("bun", "Bun", "JS/TS"),
    Sig("deno", "Deno", "TS"),
    Sig("node", "Node.js", "JS"),
    Sig("uvicorn", "FastAPI", "Python"),
    Sig("fastapi", "FastAPI", "Python"),
    Sig("gunicorn", "Gunicorn", "Python"),
    Sig("flask", "Flask", "Python"),
    Sig("django", "Django", "Python"),
    Sig("manage.py", "Django", "Python"),
    Sig("streamlit", "Streamlit", "Python"),
    Sig("gradio", "Gradio", "Python"),
    Sig("python", "Python", "Python"),
    Sig("flink", "Flink", "Java"),
    Sig("spark", "Spark", "Java"),
    Sig("spring", "Spring", "Java"),
    Sig("gradle", "Gradle", "Java"),
    Sig("mvn", "Maven", "Java"),
    Sig("java", "Java", "Java"),
    Sig("kotlin", "Kotlin", "Kotlin"),
    Sig("scala", "Scala", "Scala"),
    Sig("go run", "Go", "Go"),
    Sig("cargo", "Cargo", "Rust"),
    Sig("rails", "Rails", "Ruby"),
    Sig("puma", "Puma", "Ruby"),
    Sig("ruby", "Ruby", "Ruby"),
    Sig("artisan", "Laravel", "PHP"),
    Sig("php", "PHP", "PHP"),
    Sig("mix phx", "Phoenix", "Elixir"),
    Sig("elixir", "Elixir", "Elixir"),
];

struct DirMarker(&'static str, &'static str, &'static str);

const DIR_MARKERS: &[DirMarker] = &[
    DirMarker("next.config.js", "Next.js", "JS/TS"),
    DirMarker("next.config.mjs", "Next.js", "JS/TS"),
    DirMarker("next.config.ts", "Next.js", "JS/TS"),
    DirMarker("vite.config.ts", "Vite", "JS/TS"),
    DirMarker("vite.config.js", "Vite", "JS/TS"),
    DirMarker("nuxt.config.ts", "Nuxt", "JS/TS"),
    DirMarker("angular.json", "Angular", "TS"),
    DirMarker("svelte.config.js", "SvelteKit", "JS/TS"),
    DirMarker("astro.config.mjs", "Astro", "JS/TS"),
    DirMarker("manage.py", "Django", "Python"),
    DirMarker("pyproject.toml", "Python", "Python"),
    DirMarker("Cargo.toml", "Rust", "Rust"),
    DirMarker("go.mod", "Go", "Go"),
    DirMarker("Gemfile", "Ruby", "Ruby"),
    DirMarker("composer.json", "PHP", "PHP"),
    DirMarker("mix.exs", "Elixir", "Elixir"),
    DirMarker("build.gradle", "Gradle", "Java"),
    DirMarker("pom.xml", "Maven", "Java"),
    DirMarker("package.json", "Node.js", "JS/TS"),
];

fn detect(cmdline: &str, cwd: Option<&str>) -> (&'static str, &'static str) {
    let lower = cmdline.to_lowercase();
    for s in CMD_SIGS {
        if lower.contains(s.0) {
            return (s.1, s.2);
        }
    }
    if let Some(dir) = cwd {
        for m in DIR_MARKERS {
            if Path::new(dir).join(m.0).exists() {
                return (m.1, m.2);
            }
        }
    }
    ("Unknown", "Unknown")
}

fn project_name(cwd: Option<&str>) -> String {
    cwd.and_then(|p| Path::new(p).file_name())
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "—".into())
}

// ── Start command detection ─────────────────────────────────────────────────

/// Walk up the parent chain to find the original start command.
/// e.g. next-server → node .../next dev → bun run dev
fn find_start_cmd(pid: u32, ps_map: &HashMap<u32, ProcInfo>, cwd: Option<&str>) -> String {
    // Collect the chain: self → parent → grandparent (max 5 levels)
    let mut chain: Vec<u32> = Vec::new();
    let mut current = pid;
    for _ in 0..5 {
        chain.push(current);
        if let Some(info) = ps_map.get(&current) {
            if info.ppid <= 1 || info.ppid == current {
                break;
            }
            current = info.ppid;
        } else {
            break;
        }
    }

    // Walk the chain from the child up looking for a meaningful command.
    // Prefer: script runner (bun/npm/yarn/pnpm/deno) > node with args > package.json match > fallback

    // 1) Check if any ancestor IS a script runner with a "run" command
    for &p in chain.iter().rev() {
        if let Some(info) = ps_map.get(&p) {
            let name = &info.name;
            let cmd = info.cmdline.trim();
            // bun run dev, npm run start, yarn dev, pnpm run dev, deno task dev
            if matches!(name.as_str(), "bun" | "npm" | "yarn" | "pnpm" | "npx" | "deno") {
                return clean_cmd(cmd);
            }
        }
    }

    // 2) Check if any ancestor is "node" running a recognizable script
    for &p in chain.iter().rev() {
        if let Some(info) = ps_map.get(&p) {
            if info.name == "node" {
                let cmd = info.cmdline.trim();
                // node /path/to/.bin/next dev --turbopack → next dev --turbopack
                let cleaned = clean_cmd(cmd);
                if cleaned != "node" && !cleaned.contains("(v") {
                    return cleaned;
                }
            }
        }
    }

    // 3) Try package.json script matching against parent chain cmdlines
    if let Some(dir) = cwd {
        for &p in &chain {
            if let Some(info) = ps_map.get(&p) {
                if let Some(script) = match_package_script(&info.cmdline, dir) {
                    return script;
                }
            }
        }
    }

    // 4) Fallback: clean up the process's own cmdline
    if let Some(info) = ps_map.get(&pid) {
        return clean_cmd(info.cmdline.trim());
    }

    "?".into()
}

/// Clean a command for display: use basenames for binary and script paths
fn clean_cmd(cmd: &str) -> String {
    let parts: Vec<&str> = cmd.splitn(2, char::is_whitespace).collect();
    let binary_path = parts[0];
    let args = if parts.len() > 1 { parts[1].trim() } else { "" };

    let bin = Path::new(binary_path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| binary_path.to_string());

    if args.is_empty() {
        return bin;
    }

    // For interpreters, also clean the first argument (script path)
    if matches!(bin.as_str(), "node" | "python" | "python3" | "ruby" | "php" | "java" | "perl") {
        let arg_parts: Vec<&str> = args.splitn(2, char::is_whitespace).collect();
        let first_arg = arg_parts[0];
        let rest = if arg_parts.len() > 1 { arg_parts[1] } else { "" };

        // Skip if first arg looks like version info, not a path
        if first_arg.starts_with('(') {
            return bin;
        }

        let clean_first = if first_arg.contains('/') && !first_arg.starts_with('-') {
            Path::new(first_arg)
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| first_arg.to_string())
        } else {
            first_arg.to_string()
        };

        return if rest.is_empty() {
            format!("{bin} {clean_first}")
        } else {
            format!("{bin} {clean_first} {rest}")
        };
    }

    format!("{bin} {args}")
}

/// Try to match the running cmdline against package.json scripts
fn match_package_script(cmdline: &str, cwd: &str) -> Option<String> {
    let content = std::fs::read_to_string(Path::new(cwd).join("package.json")).ok()?;
    let json: serde_json::Value = serde_json::from_str(&content).ok()?;
    let scripts = json.get("scripts")?.as_object()?;

    let cmd_lower = cmdline.to_lowercase();

    let mut best: Option<(&str, usize)> = None;

    for (name, value) in scripts {
        if let Some(script_val) = value.as_str() {
            let words: Vec<&str> = script_val
                .split_whitespace()
                .filter(|w| !w.starts_with('-') && !w.starts_with('$'))
                .take(2)
                .collect();

            if words.is_empty() {
                continue;
            }

            let matched = words
                .iter()
                .filter(|w| cmd_lower.contains(&w.to_lowercase()))
                .count();

            if matched == words.len() && best.map(|(_, s)| matched > s).unwrap_or(true) {
                best = Some((name.as_str(), matched));
            }
        }
    }

    let (script_name, _) = best?;
    let pm = detect_package_manager(cmdline, cwd);
    Some(format!("{pm} run {script_name}"))
}

fn detect_package_manager(cmdline: &str, cwd: &str) -> &'static str {
    let lower = cmdline.to_lowercase();
    if lower.contains("bun") {
        return "bun";
    }
    if lower.contains("yarn") {
        return "yarn";
    }
    if lower.contains("pnpm") {
        return "pnpm";
    }

    let dir = Path::new(cwd);
    if dir.join("bun.lockb").exists() || dir.join("bun.lock").exists() {
        return "bun";
    }
    if dir.join("yarn.lock").exists() {
        return "yarn";
    }
    if dir.join("pnpm-lock.yaml").exists() {
        return "pnpm";
    }
    "npm"
}

// ── Public API ──────────────────────────────────────────────────────────────

/// Scan ALL listening TCP ports (no filter), with process info
pub fn scan_all_ports() -> Vec<PortInfo> {
    let port_pid = get_listening_ports();
    let all: Vec<(u16, u32)> = port_pid.into_iter().collect();
    build_port_infos(all, false)
}

pub fn scan_ports(known_only: bool) -> Vec<PortInfo> {
    let port_pid = get_listening_ports();

    let filtered: Vec<(u16, u32)> = port_pid
        .into_iter()
        .filter(|&(port, _)| (1024..=65000).contains(&port))
        .collect();

    build_port_infos(filtered, known_only)
}

fn build_port_infos(ports: Vec<(u16, u32)>, known_only: bool) -> Vec<PortInfo> {
    let unique_pids: Vec<u32> = {
        let mut pids: Vec<u32> = ports.iter().map(|&(_, pid)| pid).collect();
        pids.sort_unstable();
        pids.dedup();
        pids
    };

    // Call #2 & #3: run ps (all processes for parent chain) and lsof (cwd) in parallel
    let pids_for_cwd = unique_pids;
    let ps_handle = std::thread::spawn(batch_ps_all);
    let cwd_handle = std::thread::spawn(move || batch_cwd(&pids_for_cwd));

    let ps_info = ps_handle.join().unwrap_or_default();
    let cwd_info = cwd_handle.join().unwrap_or_default();

    let mut results: Vec<PortInfo> = ports
        .iter()
        .map(|&(port, pid)| {
            let pi = ps_info.get(&pid);
            let cwd = cwd_info.get(&pid).cloned();

            let name = pi.map(|p| p.name.clone()).unwrap_or_else(|| "?".into());
            let cmdline = pi.map(|p| p.cmdline.clone()).unwrap_or_else(|| "?".into());
            let user = pi.map(|p| p.user.clone()).unwrap_or_else(|| "?".into());

            let (framework, language) = detect(&cmdline, cwd.as_deref());
            let project = project_name(cwd.as_deref());
            let start_cmd = find_start_cmd(pid, &ps_info, cwd.as_deref());

            PortInfo {
                port,
                pid,
                name,
                cmdline,
                start_cmd,
                cwd,
                project,
                framework: framework.to_string(),
                language: language.to_string(),
                user,
            }
        })
        .collect();

    results.sort_by_key(|p| p.port);

    if known_only {
        results.retain(|r| r.framework != "Unknown");
        let mut seen = HashMap::new();
        results.retain(|r| seen.insert(r.pid, true).is_none());
    }

    results
}

pub fn scan_port(port: u16) -> Option<PortInfo> {
    let port_pid = get_listening_ports();
    let &pid = port_pid.get(&port)?;

    let ps = batch_ps_all();
    let cwd_map = batch_cwd(&[pid]);

    let pi = ps.get(&pid);
    let cwd = cwd_map.get(&pid).cloned();

    let name = pi.map(|p| p.name.clone()).unwrap_or_else(|| "?".into());
    let cmdline = pi.map(|p| p.cmdline.clone()).unwrap_or_else(|| "?".into());
    let user = pi.map(|p| p.user.clone()).unwrap_or_else(|| "?".into());

    let (framework, language) = detect(&cmdline, cwd.as_deref());
    let project = project_name(cwd.as_deref());
    let start_cmd = find_start_cmd(pid, &ps, cwd.as_deref());

    Some(PortInfo {
        port,
        pid,
        name,
        cmdline,
        start_cmd,
        cwd,
        project,
        framework: framework.to_string(),
        language: language.to_string(),
        user,
    })
}

pub fn kill_port(port: u16, force: bool) -> (bool, String) {
    let info = match scan_port(port) {
        Some(i) => i,
        None => return (false, format!("No process found on port {port}")),
    };

    use nix::sys::signal::{self, Signal};
    use nix::unistd::Pid;

    let sig = if force { Signal::SIGKILL } else { Signal::SIGTERM };
    match signal::kill(Pid::from_raw(info.pid as i32), sig) {
        Ok(()) => (
            true,
            format!(
                "Killed PID {} ({} — {}) on port {}",
                info.pid, info.framework, info.project, port
            ),
        ),
        Err(e) => (false, format!("Failed to kill PID {}: {}", info.pid, e)),
    }
}
