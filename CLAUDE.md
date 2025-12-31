# Emdash Development Guide

This file provides context and best practices for working with the Emdash codebase using Claude Code.

## 🚨 CRITICAL RULES

- **NEVER modify** `drizzle/meta/` or numbered migration files without explicit coordination
- **NEVER commit** secrets, API keys, or user data
- **NEVER modify** signing/notarization settings without review
- **ALWAYS** run `bun run type-check` and `bun run lint` before committing
- **ALWAYS** test changes in both core and renderer paths
- **ALWAYS** use worktrees for feature development (never commit directly to `main`) !!!!!

## 📋 Project Context

**Emdash** is a cross-platform Tauri application that orchestrates multiple coding agents (Claude Code, Codex, etc.) in parallel. Each agent runs in its own Git worktree to keep changes isolated.

### Architecture

- **App Core** (`src-tauri/src/`): Tauri commands, IPC handlers, services (Git, worktrees, agents, database)
- **Renderer Process** (`src/renderer/`): React UI built with Vite, hooks, components
- **Shared** (`src/shared/`): Shared utilities and type definitions
- **Database**: SQLite via Drizzle ORM, stored in OS userData folder
- **Worktrees**: Created in sibling `worktrees/` directory (outside repo root)

### Tech Stack

- **Runtime**: Tauri 2.x + Rust, Node.js 20.0.0+ (recommended: 22.20.0) for tooling
- **Package manager**: Bun 1.3+
- **Frontend**: React 18, TypeScript 5.3, Vite 5, Tailwind CSS 3
- **Backend**: Rust (Tauri commands), Drizzle ORM, SQLite

## 🔧 Bash Commands

### Development

```bash
# Quick start: install dependencies and run dev server
bun run d

# Or separately:
bun install          # Install dependencies
bun run dev          # Run Tauri dev (spawns renderer via tauri.conf)
bun run dev:renderer # Run Vite dev server only
```

### Building & Testing

```bash
bun run build        # Build Tauri app + renderer bundle
bun run build:renderer # Build renderer only

bun run type-check   # TypeScript type checking
bun run lint         # ESLint
bun run format       # Format with Prettier
bun run format:check # Check formatting

bunx vitest run       # Run tests (tests in src/**/*.test.ts)
```

### Packaging

```bash
bun run build        # Builds installers/bundles via Tauri
```

## 📐 Code Style Guidelines

### TypeScript

- Use **strict TypeScript** (`strict: true` in tsconfig)
- Prefer **explicit types** over `any`; use `unknown` when type is truly unknown
- Use **type imports** for types: `import type { Foo } from './bar'`
- Use **interfaces** for object shapes, **types** for unions/intersections
- **NO** `@ts-ignore` or `@ts-expect-error` without explanation

### React Components

- Use **functional components** with hooks (`React.FC` or direct function)
- **Named exports** preferred: `export function ComponentName() {}`
- **Client components** must have `'use client'` directive (if needed)
- Use **Tailwind CSS** for styling (utility classes, not inline styles)
- Use **lucide-react** for icons
- Use **Radix UI** primitives for complex UI components

### File Organization

- **App core**: `src-tauri/src/`
- **Renderer**: `src/renderer/components/`, `src/renderer/hooks/`, `src/renderer/lib/`
- **Shared**: `src/shared/` for code used by both processes
- **Types**: `src/types/` for global type definitions
- Use **kebab-case** for file names: `workspace-terminal-panel.tsx`
- Use **PascalCase** for component files: `WorkspaceTerminalPanel.tsx`

### React Patterns

- **Hooks**: Always call hooks at the top level (no conditional hooks)
- **State**: Use `useState` for local state, context for shared state
- **Effects**: Clean up subscriptions/event listeners in `useEffect` return
- **Memoization**: Use `useMemo`/`useCallback` sparingly (only when needed)
- **Refs**: Use `useRef` for DOM refs or values that don't trigger re-renders

### IPC Communication

- **Core → Renderer**: Emit Tauri events (e.g. `app.emit('event-name', data)`)
- **Renderer → Core**: Use `invoke('command_name', args)` from `@tauri-apps/api`
- **Event listeners**: Always clean up in `useEffect` return
- **Type safety**: Define IPC types in `src/renderer/types/desktop-api.d.ts`

### Error Handling

- **Core (Tauri)**: Use Rust logging (`eprintln!`, `log`, or tracing)
- **Renderer**: Use `console.error()` or toast notifications
- **IPC errors**: Return `{ success: false, error: string }` format
- **User-facing errors**: Show friendly messages, log technical details

## 🗄️ Database & Migrations

- **ORM**: Drizzle ORM with SQLite
- **Migrations**: Generated in `drizzle/` directory
- **NEVER** manually edit `drizzle/meta/` or numbered migration files
- **Schema changes**: Modify `src/shared/db/schema.ts`, then run `drizzle-kit generate`
- **Migration files**: Commit migration SQL files, but coordinate on schema changes

## 🌳 Git Workflow

### Branch Strategy

- **Default branch**: `main` (NEVER commit directly)
- **Feature branches**: Create from `main`, use descriptive names
- **Worktrees**: Use for parallel agent work (created automatically)

### Worktree Pattern

- Worktrees created in sibling `worktrees/` directory
- Each workspace gets its own worktree with unique branch
- Worktree paths: `../worktrees/{workspace-name}-{timestamp}`
- **IMPORTANT**: Agents run in worktree directories, not base repo

### Commit Messages

- Use conventional commits: `feat:`, `fix:`, `refactor:`, `docs:`, `chore:`
- Be descriptive but concise
- Reference issues when applicable: `fix(agent): resolve worktree path issue (#123)`

## 🧪 Testing

- **Test files**: `src/**/*.test.ts` (Vitest)
- **Run tests**: `bunx vitest run`
- **Test patterns**: Unit tests for utilities, integration tests for services
- **Mocking**: Mock desktop bridge APIs and external services
- **Coverage**: Aim for critical paths (IPC handlers, services)

## 🚀 Development Environment Setup

### Prerequisites

```bash
# Node.js (use nvm)
nvm use  # Installs Node 22.20.0 if missing

# Bun (package manager/runtime)
bun --version  # Ensure Bun 1.3+

# Optional but recommended:
npm install -g @openai/codex  # For Codex agent testing
npm install -g @anthropic-ai/claude-code  # For Claude Code testing
brew install gh  # GitHub CLI for GitHub integration
```

### First Time Setup

```bash
git clone <repo-url>
cd emdash
nvm use
bun run d  # Installs deps and starts Tauri dev server
```

### Hot Reload

- **Renderer changes**: Hot-reloads automatically via Vite
- **Core changes**: Require app restart

## 🔍 Common Patterns

### IPC Command Pattern

```rust
// src-tauri/src/example.rs
use serde_json::{json, Value};

#[tauri::command]
fn example_action(id: String) -> Value {
  json!({ "success": true, "data": { "id": id } })
}
```

```typescript
// Renderer (src/renderer/components/Example.tsx)
const result = await window.desktopAPI.exampleAction({ id: '123' });
if (result.success) {
  // Handle success
} else {
  // Handle error
}
```

### Core Service Pattern

```rust
// src-tauri/src/example_service.rs
pub fn do_something(id: &str) -> Result<String, String> {
  Ok(id.to_string())
}
```

### React Hook Pattern

```typescript
// src/renderer/hooks/useExample.ts
export function useExample(id: string) {
  const [data, setData] = useState(null);
  const [loading, setLoading] = useState(true);
  
  useEffect(() => {
    // Fetch data via IPC
  }, [id]);
  
  return { data, loading };
}
```

## ⚠️ Common Pitfalls

1. **PTY Resize Errors**: PTYs must be cleaned up on exit. Use `removePty()` in exit handlers.
2. **Worktree Path Resolution**: Always resolve worktree paths from `WorktreeService` when creating agents.
3. **React Hooks Rules**: Never call hooks conditionally or after early returns.
4. **IPC Type Safety**: Always define types in `desktop-api.d.ts` for bridge methods.
6. **Database Migrations**: Never manually edit migration files; use Drizzle Kit.
6. **Context Sections**: When working with diffs, ensure context sections are properly collapsed/expanded.

## 📚 Key Files & Utilities

### Core Services

- `src-tauri/src/worktree.rs` - Git worktree management
- `src-tauri/src/github.rs` - GitHub auth + repo/issue/PR workflows
- `src-tauri/src/git.rs` - Git status/diff/PR generation helpers
- `src-tauri/src/db.rs` - Database operations (SQLite)
- `src-tauri/src/pty.rs` - PTY (terminal) management
- `src-tauri/src/container.rs` - Container runs + preview ports

### Core Components

- `src/renderer/components/ChatInterface.tsx` - Main chat interface
- `src/renderer/components/MultiAgentWorkspace.tsx` - Multi-agent workspace UI
- `src/renderer/components/RightSidebar.tsx` - File changes and terminal panel
- `src/renderer/components/ChangesDiffModal.tsx` - Diff viewer with syntax highlighting

### Utilities

- `src/renderer/lib/utils.ts` - General utilities
- `src/renderer/lib/languageUtils.ts` - Language detection for syntax highlighting

## 🎯 Workflow Best Practices

### When Adding Features

1. **Plan first**: Understand the architecture and where your code fits
2. **Create branch**: `git checkout -b feat/feature-name`
3. **Implement**: Follow code style guidelines
4. **Test**: Run type-check, lint, and tests
5. **Commit**: Use conventional commit messages
6. **PR**: Create PR with clear description

### When Fixing Bugs

1. **Reproduce**: Understand the issue fully
2. **Fix**: Make minimal changes to fix the issue
3. **Test**: Verify fix works and doesn't break other things
4. **Document**: Add comments if fix is non-obvious

### When Refactoring

1. **Small steps**: Break refactoring into small, testable commits
2. **Preserve behavior**: Ensure functionality remains the same
3. **Update tests**: Update tests to match new structure
4. **Type safety**: Maintain or improve type safety

## 🔐 Security & Privacy

- **Secrets**: Never commit API keys, tokens, or credentials
- **User data**: Database stored locally in OS userData folder
- **Logs**: Agent logs stored outside repos (in userData/logs/)
- **IPC**: Validate all IPC inputs, sanitize user-provided data

## 📝 Documentation

- **Code comments**: Add JSDoc comments for public APIs
- **README**: Update README.md for user-facing changes
- **CONTRIBUTING**: Follow CONTRIBUTING.md for development workflow
- **Changelog**: Consider updating changelog for significant changes

## 🐛 Debugging Tips

- **Core logs**: Check terminal where `bun run dev` runs
- **Renderer logs**: Use webview DevTools (Cmd/Ctrl+Alt+I in dev builds)
- **IPC debugging**: Add `println!`/`eprintln!` or structured logs in Tauri commands
- **Database**: SQLite file location logged on startup
- **Worktrees**: Check `worktrees/` directory for created worktrees
- **PTY issues**: Check `pty.rs` logs and lifecycle events

## 🎨 UI/UX Guidelines

- **Dark mode**: Support both light and dark themes
- **Accessibility**: Use semantic HTML, ARIA labels where needed
- **Responsive**: Ensure UI works at different window sizes
- **Loading states**: Show loading indicators for async operations
- **Error states**: Display user-friendly error messages
- **Toast notifications**: Use toast for non-blocking notifications

---

**Remember**: This is a living document. Update it as patterns evolve and new best practices emerge. Use `#` in Claude Code to automatically incorporate improvements into this file.
