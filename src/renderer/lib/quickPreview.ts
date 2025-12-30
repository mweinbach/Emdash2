import {
  startContainerRun,
  subscribeToTaskRunState,
  getContainerRunState,
} from '@/lib/containerRuns';

declare const window: Window & {
  electronAPI: any;
};

export async function isNodeProject(taskPath?: string): Promise<boolean> {
  if (!taskPath) return false;
  try {
    const res = await window.electronAPI.fsRead(taskPath, 'package.json', 64 * 1024);
    return !!(res?.success && typeof res.content === 'string' && res.content.includes('{'));
  } catch {
    return false;
  }
}

async function detectPackageManager(taskPath?: string): Promise<'bun' | 'npm'> {
  if (!taskPath) return 'npm';
  try {
    const bunLock = await window.electronAPI.fsRead(taskPath, 'bun.lockb', 1);
    if (bunLock?.success) return 'bun';
  } catch {}
  try {
    const bunTextLock = await window.electronAPI.fsRead(taskPath, 'bun.lock', 1);
    if (bunTextLock?.success) return 'bun';
  } catch {}
  return 'npm';
}

export async function ensureCompose(taskPath?: string): Promise<boolean> {
  if (!taskPath) return false;
  try {
    const candidates = ['docker-compose.yml', 'docker-compose.yaml', 'compose.yml', 'compose.yaml'];
    for (const file of candidates) {
      const res = await window.electronAPI.fsRead(taskPath, file, 1);
      if (res?.success) return true;
    }
  } catch {}
  // Write a minimal compose to run dev quickly
  const pm = await detectPackageManager(taskPath);
  const content =
    pm === 'bun'
      ? `services:\n  web:\n    image: oven/bun:1.3.5\n    working_dir: /workspace\n    volumes:\n      - ./:/workspace\n    environment:\n      - HOST=0.0.0.0\n      - PORT=3000\n    command: bash -lc \"if [ -f bun.lockb ] || [ -f bun.lock ]; then bun install --frozen-lockfile; else bun install; fi && bun run dev\"\n    expose:\n      - \"3000\"\n      - \"5173\"\n      - \"8080\"\n      - \"8000\"\n`
      : `services:\n  web:\n    image: node:20\n    working_dir: /workspace\n    volumes:\n      - ./:/workspace\n    environment:\n      - HOST=0.0.0.0\n      - PORT=3000\n    command: bash -lc \"if [ -f package-lock.json ]; then npm ci; else npm install --no-package-lock; fi && npm run dev\"\n    expose:\n      - \"3000\"\n      - \"5173\"\n      - \"8080\"\n      - \"8000\"\n`;
  try {
    const res = await window.electronAPI.fsWriteFile(
      taskPath,
      'docker-compose.yml',
      content,
      false
    );
    return !!res?.success;
  } catch {
    return false;
  }
}

export async function quickStartPreview(args: {
  taskId: string;
  taskPath: string;
  onPreviewUrl?: (url: string) => void;
}): Promise<{ ok: boolean; error?: string }> {
  const { taskId, taskPath, onPreviewUrl } = args;
  try {
    const node = await isNodeProject(taskPath);
    if (!node) return { ok: false, error: 'Not a Node.js project (no package.json).' };
    await ensureCompose(taskPath);
    await startContainerRun({ taskId, taskPath, mode: 'container' });
    // If already have a preview, use it immediately
    const existing = getContainerRunState(taskId);
    if (existing?.previewUrl && onPreviewUrl) onPreviewUrl(existing.previewUrl);
    // Subscribe for preview becoming ready
    const unsubRef: { current: null | (() => void) } = { current: null };
    await new Promise<void>((resolve) => {
      unsubRef.current = subscribeToTaskRunState(taskId, (state) => {
        if (state.previewUrl) {
          onPreviewUrl?.(state.previewUrl);
          resolve();
        }
      });
      // Safety timeout
      setTimeout(() => resolve(), 60_000);
    });
    if (unsubRef.current)
      try {
        unsubRef.current();
      } catch {}
    return { ok: true };
  } catch (e: any) {
    return { ok: false, error: e?.message || String(e) };
  }
}
