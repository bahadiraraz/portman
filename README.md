# portman

Interactive TUI tool to see what's running on your dev ports — and kill it.

<img width="1538" height="936" alt="image" src="https://github.com/user-attachments/assets/b678eb06-4e89-447d-aed6-17cf726bf69a" />

## Features

- **Instant overview** — see all listening ports with project name, framework, language, and PID
- **Smart detection** — auto-detects Next.js, Vite, FastAPI, Django, Flink, Spring, and 30+ frameworks
- **Start command** — shows `bun run dev`, `npm run start`, etc. by walking the process tree
- **Kill processes** — SIGTERM or SIGKILL directly from the TUI
- **Search** — press `/` to fuzzy-search by port, project, framework, or command
- **Filter tabs** — filter by language (JS/TS, Python, Java, Go, Rust…) — only shows languages that are running
- **Non-blocking** — port scanning runs in a background thread, UI never freezes
- **~1MB binary** — fast startup, zero runtime dependencies
- **macOS & Linux** — works anywhere `lsof` and `ps` are available

## Install

### Homebrew (macOS & Linux)

```sh
brew install bahadiraraz/tap/portman
```

### Cargo (from source)

```sh
cargo install portman
```

### Pre-built binaries

Download from [GitHub Releases](https://github.com/bahadiraraz/portman/releases).

## Usage

### Interactive TUI (default)

```sh
portman
```

| Key | Action |
|-----|--------|
| `↑↓` | Navigate ports |
| `→` / `Enter` | Open actions panel |
| `←` / `Esc` | Go back |
| `/` | Search |
| `Tab` / `Shift+Tab` | Cycle filters |
| `q` | Quit |

### CLI commands

```sh
# List dev ports
portman list

# List all ports (including non-dev)
portman list --all

# Show ALL listening TCP ports with process info (like netstat, but better)
portman ps

# JSON output
portman list --json
portman ps --json

# Info for a specific port
portman info 3000

# Kill a port
portman kill 3000
portman kill 3000 --force
```

## Detected Frameworks

| Language | Frameworks |
|----------|-----------|
| JS/TS | Next.js, Vite, Nuxt, Remix, Astro, Angular, SvelteKit, Express, NestJS, Bun, Deno |
| Python | FastAPI, Django, Flask, Gunicorn, Streamlit, Gradio |
| Java | Spring, Flink, Spark, Gradle, Maven |
| Go | Go |
| Rust | Cargo |
| Ruby | Rails, Puma |
| PHP | Laravel |
| Elixir | Phoenix |

## How It Works

1. `lsof -iTCP -sTCP:LISTEN` to find all listening ports (1 subprocess call)
2. `ps -axo pid,ppid,user,args` to get process info + parent chain (1 call)
3. `lsof -d cwd` to get working directories (1 call, parallel with #2)
4. Framework detection via command-line patterns + directory markers (`next.config.js`, `pyproject.toml`, etc.)
5. Start command detection by walking the parent process chain (e.g., `next-server` -> `node next dev` -> `bun run dev`)
6. `package.json` script matching + lock file detection for package manager

Total: **3 subprocess calls**, parallelized. Scans complete in ~200ms.

## License

MIT
