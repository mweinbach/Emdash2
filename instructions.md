# BUILD_STEPS.md — Electron Multi-Agent Terminal (High-Level)

Goal: Electron app that opens **multiple pseudo-terminals**, one per **Git worktree/branch**, and lets users **invoke CLI assistants** (aider/ollama/anthropic/openai). **Local-only, BYO keys.**

---

## Prereqs

- Node 20+, Git 2.38+, Bun (or npm/pnpm/yarn)
- Electron + Vite scaffold (or create one)
- Optional CLIs installed on PATH: `aider`, `ollama`, `anthropic`, `openai`

---

## Step 1 — Scaffold & Preload

**Tasks**

- Create or use an Electron+Vite project.
- In `BrowserWindow`, enable `preload`, `contextIsolation:true`, `nodeIntegration:false`.
- In `preload.ts`, `contextBridge.exposeInMainWorld('ipc', { invoke, on })`.

**Done when** renderer can `window.ipc.invoke('ping')` and get `"pong"` via `ipcMain.handle('ping', ...)`.

---

## Step 2 — PTY & Terminal UI Basics

**Tasks**

- Install: `bun add node-pty xterm && bun add -d @types/node`
- Create `src/main/ptyManager.ts` with:
  - `startPty(id, cwd, shell?) -> IPty` (stores in `Map`)
  - `writePty(id, data)`, `resizePty(id, cols, rows)`, `killPty(id)`
- Create `src/main/ipcPty.ts` with IPC:
  - `pty:start` (spawns, pipes `onData` → `webContents.send('pty:data:<id>', chunk)`, `onExit` → `pty:exit:<id>`)
  - `pty:input`, `pty:resize`, `pty:kill`
- In renderer, make `TerminalPane` using **xterm.js**:
  - `term.open(el)`
  - `ipc.invoke('pty:start', { id, cwd })`
  - Listen: `pty:data:<id>`, `pty:exit:<id>`
  - `term.onData(d => ipc.invoke('pty:input', { id, data: d }))`

**Done when** you can run `ls`/`git status` in the terminal.

---

## Step 3 — Worktrees (Branch Isolation)

**Tasks**

- Add `src/main/git.ts` with `ensureWorktree(mainRepo, name, branch): Promise<string>` using `child_process.execFile('git', ['worktree','add', worktreePath, branch], { cwd: mainRepo })`. Reuse if exists.
- UI “New Session” modal:
  - Pick **main repo root**, enter **branch**, optional **worktree name** (default = branch).
  - Call `ensureWorktree` → returns absolute path → pass as `cwd` to `pty:start`.

**Done when** terminal `git status` shows the selected branch per session.

---

## Step 4 — Invoke CLI Assistants (Inside PTY)

**Tasks**

- Add `ipcMain.handle('agent:launchCli', ({ id, cmd, args }))` → `writePty(id, \`${cmd} ${args.join(' ')}\r\`)`.
- UI: “Launch CLI” button with presets (`aider`, `ollama`, `anthropic`, `openai`) + freeform args.
- Optional: PATH check (`which`/`where`) → show install tips if missing.

**Done when** pressing the button starts the chosen CLI inside that session’s shell.

---

## Step 5 — Multiple Sessions/Tabs

**Tasks**

- Simple session store: `{ id, label, cwd, cli? }`.
- Render one `TerminalPane` per session (tabs or splits).
- Controls per session: **Kill**, **Clear**, **Resize to fit**.

**Done when** two sessions run concurrently, isolated by worktrees.

---

## Step 6 — BYO Keys (Local Env)

**Tasks**

- Dev only: `dotenv` in **main**; ship `.env.example` with commented keys (e.g., `ANTHROPIC_API_KEY=`, `OPENAI_API_KEY=`).
- Pass `env` to `node-pty.spawn` (merge `process.env` + optional per-session overrides stored locally).
- Settings UI: allow per-session env overrides (never send secrets to renderer except for display with masking).

**Done when** `echo $ANTHROPIC_API_KEY` (or platform equivalent) shows the value when set.

---

## Step 7 — Quality-of-Life

**Tasks**

- Resize: call `pty.resize()` on container resize (e.g., via `ResizeObserver`).
- Scrollback: 5k–10k lines in xterm.
- Save logs: stream PTY output to `${worktree}/.agentlogs/session-YYYYMMDD-HHMM.log`.
- Exit handling: show `[Process exited]`, disable input.

**Done when** resizing feels natural, logs are written, exits are clear.

---

## Step 8 — Packaging & OSS Hygiene

**Tasks**

- Add `LICENSE` (MIT), `.env.example`, `.gitignore` for `.env`, `CONTRIBUTING.md`, `SECURITY.md`.
- Configure packaging (`electron-builder`/`electron-vite`); rebuild native deps if needed.
- Smoke test packaged app on your target OSes.

**Done when** packaged app launches, terminals work, CLIs spawn.

---

## Minimal “Ask Codex” Prompts (per step)

- **Step 1**: “Add preload and secure IPC in Electron+Vite; expose `ipc.invoke`/`ipc.on`; implement a ping handler.”
- **Step 2**: “Add node-pty in main, xterm in renderer; wire IPC to echo shell output and keystrokes.”
- **Step 3**: “Implement `ensureWorktree(mainRepo, name, branch)` using `git worktree`; add a modal to create and start a session.”
- **Step 4**: “Add `agent:launchCli` IPC that types a command into the PTY; add UI with CLI presets and args.”
- **Step 5**: “Create a session store and tabbed UI; mount one TerminalPane per session.”
- **Step 6**: “Load .env in main; pass env to `node-pty.spawn`; add per-session env overrides in settings.”
- **Step 7**: “Implement PTY resize, scrollback config, session log writing, and exit handling.”
- **Step 8**: “Add MIT license, .env.example, and packaging; ensure node-pty rebuild works in CI.”

---

## Gotchas (read once)

- **node-pty** native ABI must match Electron: use `electron-rebuild` if needed.
- **macOS GUI PATH** may be empty: set PATH in `env` or use login shell.
- **Windows quoting**: keep args simple or wrap in `.cmd`/`.sh`.
- **Permissions**: ensure worktree dirs are writable.
- **Never auto-merge** branches; keep changes per-agent, PR manually.

---
