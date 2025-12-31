<img width="4856" height="1000" alt="Github_banner (1)" src="https://github.com/user-attachments/assets/9dfdd1fd-ea4f-4b86-9daf-d4a46952594a" />

<div align="center" style="margin:24px 0;">

<br />

# Emdash2 (macOS)

> **Emdash2 is a fork of [generalaction/emdash](https://github.com/generalaction/emdash).**  
> Full credit to the upstream Emdash project and its contributors.

**Run multiple coding agents in parallel**

Emdash2 is a macOS-focused fork of Emdash with a Tauri + Rust desktop shell and a Ghostty-powered terminal.

Emdash2 lets you develop and test multiple features with multiple agents in parallel. It’s provider-agnostic (supports 15+ CLI agents, such as Claude Code, Qwen Code, Amp, and Codex) and runs each agent in its own Git worktree to keep changes clean; Hand off Linear, GitHub, or Jira tickets to an agent and review diffs side-by-side.

<div align="center" style="margin:24px 0;">

[Installation](#installation) • [Integrations](#integrations) • [Demo](#demo) • [Contributing](#contributing) • [FAQ](#faq)


# Providers

<img width="4856" height="1000" alt="integration_banner" src="https://github.com/user-attachments/assets/894c3db8-3be5-4730-ae7d-197958b0a044" />

### Supported CLI Providers

Emdash2 currently supports fifteen CLI providers and we are adding new providers regularly. If you miss one, let us know or create a PR. 

| CLI Provider | Status | Install |
| ----------- | ------ | ----------- |
| [Codex](https://developers.openai.com/codex/cli/) | ✅ Supported | `npm install -g @openai/codex` |
| [Amp](https://ampcode.com/manual) | ✅ Supported | `npm install -g @sourcegraph/amp@latest` |
| [Auggie](https://docs.augmentcode.com/cli/overview) | ✅ Supported | `npm install -g @augmentcode/auggie` |
| [Charm](https://github.com/charmbracelet/crush) | ✅ Supported | `npm install -g @charmland/crush` |
| [Claude Code](https://www.npmjs.com/package/%40anthropic-ai/claude-code) | ✅ Supported | `npm install -g @anthropic-ai/claude-code` |
| [Cursor](https://cursor.com/cli) | ✅ Supported | `curl https://cursor.com/install -fsS | bash` |
| [Droid](https://docs.factory.ai/cli/getting-started/quickstart) | ✅ Supported | `curl -fsSL https://app.factory.ai/cli | sh` |
| [Gemini](https://github.com/google-gemini/gemini-cli) | ✅ Supported | `npm install -g @google/gemini-cli` |
| [GitHub Copilot](https://docs.github.com/en/copilot/how-tos/set-up/installing-github-copilot-in-the-cli) | ✅ Supported | `npm install -g @github/copilot` |
| [Goose](https://github.com/block/goose) | ✅ Supported | `curl -fsSL https://github.com/block/goose/releases/download/stable/download_cli.sh | bash` |
| [OpenCode](https://opencode.ai/docs/) | ✅ Supported | `npm install -g opencode-ai` |
| [Qwen Code](https://github.com/QwenLM/qwen-code) | ✅ Supported | `npm install -g @qwen-code/qwen-code` |
| [Kimi](https://www.kimi.com/coding/docs/en/kimi-cli.html) | ✅ Supported | `uv tool install --python 3.13 kimi-cli` |
| [Kiro](https://kiro.dev/docs/cli/) | ✅ Supported | `curl -fsSL https://cli.kiro.dev/install | bash` |
| [Rovo Dev](https://support.atlassian.com/rovo/docs/install-and-run-rovo-dev-cli-on-your-device/) | ✅ Supported | `acli rovodev auth login` |

### Issues

Emdash2 allows you to pass tickets straight from Linear, GitHub, or Jira to your coding agent. 

| Tool | Status | Authentication |
| ----------- | ------ | ----------- |
| [Linear](https://linear.app) | ✅ Supported | Connect with a Linear API key. |
| [Jira](https://www.atlassian.com/software/jira) | ✅ Supported | Provide your site URL, email, and Atlassian API token. |
| [GitHub Issues](https://docs.github.com/en/issues) | ✅ Supported | Authenticate via GitHub CLI (`gh auth login`). |

# Demo

#### Add an agents.md file
![Agents.md](https://www.emdash.sh/addagentsfilescreenshot.png)
#### Run multiple agents in parallel

![Parallel agents](https://www.emdash.sh/parallel_loop.gif)

#### Pass a Linear ticket

![Passing Linear](https://www.emdash.sh/addlinearscreenshot.png)


# Contributing

Contributions welcome! See the [Contributing Guide](CONTRIBUTING.md) to get started, and join our [Discord](https://discord.gg/f2fv7YxuR2) to discuss.

# FAQ

<details>
<summary><b>What telemetry do you collect and can I disable it?</b></summary>

> We send **anonymous, allow‑listed events** (app start/close, feature usage names, app/platform versions) to PostHog.  
> We **do not** send code, file paths, repo names, prompts, or PII.
>
> **Disable telemetry:**
>
> - In the app: **Settings → General → Privacy & Telemetry** (toggle off)
> - Or via env var before launch:
>
> ```bash
> TELEMETRY_ENABLED=false
> ```
>
> Full details: see `docs/telemetry.md`.
</details>

<details>
<summary><b>Where is my data stored?</b></summary>

> Everything is **local‑first**. We store app state in a local **SQLite** database:
>
> ```
> macOS:  ~/Library/Application Support/emdash/emdash.db
> Windows: %APPDATA%/emdash/emdash.db
> Linux:  ~/.config/emdash/emdash.db
> ```
>
> You can reset by deleting the DB (quit the app first). The file is recreated on next launch.
</details>

<details>
<summary><b>Do I need GitHub CLI?</b></summary>

> **Only if you want GitHub features** (open PRs from Emdash2, fetch repo info, GitHub Issues integration).  
> Install & sign in:
>
> ```bash
> gh auth login
> ```
>
> If you don’t use GitHub features, you can skip installing `gh`.
</details>

<details>
<summary><b>How do I add a new provider?</b></summary>

> Emdash2 is **provider‑agnostic** and built to add CLIs quickly.
>
> - Open a PR following the **Contributing Guide** (`CONTRIBUTING.md`).
> - Include: provider name, how it’s invoked (CLI command), auth notes, and minimal setup steps.
> - We’ll add it to the **Integrations** matrix and wire up provider selection in the UI.
>
> If you’re unsure where to start, open an issue with the CLI’s link and typical commands.
</details>

<details>
<summary><b>I hit a dev build crash. What’s the fast fix?</b></summary>

> 1) Reinstall dependencies:
>
> ```bash
> bun run reset
> ```
>
> 2) Then rerun:
>
> ```bash
> bun run dev
> ```
</details>

<details>
<summary><b>What permissions does Emdash2 need?</b></summary>

> - **Filesystem/Git:** to read/write your repo and create **Git worktrees** for isolation.  
> - **Network:** only for provider CLIs you choose to use (e.g., Codex, Claude) and optional GitHub actions.  
> - **Local DB:** to store your app state in SQLite on your machine.
>
> Emdash2 itself does **not** send your code or chats to any servers. Third‑party CLIs may transmit data per their policies.
</details>

[![Follow @rabanspiegel](https://img.shields.io/twitter/follow/rabanspiegel?style=social&label=Follow%20%40rabanspiegel)](https://x.com/rabanspiegel)
[![Follow @arnestrickmann](https://img.shields.io/twitter/follow/arnestrickmann?style=social&label=Follow%20%40arnestrickmann)](https://x.com/arnestrickmann)
