import type { PrCommit, PrFile } from './prStatus';

type PrChangesData = {
  commits: PrCommit[];
  files: PrFile[];
  fetchedAt: number;
};

type Listener = (data: PrChangesData | null) => void;

const cache = new Map<string, PrChangesData | null>();
const listeners = new Map<string, Set<Listener>>();
const pending = new Map<string, Promise<PrChangesData | null>>();

const DEFAULT_STALE_MS = 60_000;

async function fetchPrChanges(taskPath: string): Promise<PrChangesData | null> {
  const res = await window.desktopAPI.getPrChanges({ taskPath });
  if (!res?.success) {
    throw new Error(res?.error || 'Failed to load pull request changes.');
  }
  if (res?.pr === null) {
    return null;
  }
  return {
    commits: Array.isArray(res.commits) ? (res.commits as PrCommit[]) : [],
    files: Array.isArray(res.files) ? (res.files as PrFile[]) : [],
    fetchedAt: Date.now(),
  };
}

function isStale(entry: PrChangesData | null | undefined, staleMs: number) {
  if (!entry) return true;
  return Date.now() - entry.fetchedAt > staleMs;
}

function notify(taskPath: string, data: PrChangesData | null) {
  const taskListeners = listeners.get(taskPath);
  if (!taskListeners) return;
  for (const listener of taskListeners) {
    try {
      listener(data);
    } catch {
      // ignore listener failures
    }
  }
}

export async function refreshPrChanges(
  taskPath: string,
  opts?: { force?: boolean }
): Promise<PrChangesData | null> {
  if (!taskPath) return null;

  const inFlight = pending.get(taskPath);
  if (inFlight && !opts?.force) return inFlight;

  if (!opts?.force) {
    const cached = cache.get(taskPath);
    if (cached && !isStale(cached, DEFAULT_STALE_MS)) {
      return cached;
    }
  }

  const promise = fetchPrChanges(taskPath)
    .then((data) => {
      cache.set(taskPath, data);
      notify(taskPath, data);
      return data;
    })
    .finally(() => {
      pending.delete(taskPath);
    });

  pending.set(taskPath, promise);
  return promise;
}

export function subscribeToPrChanges(
  taskPath: string,
  listener: Listener,
  opts?: { staleMs?: number }
): () => void {
  const set = listeners.get(taskPath) || new Set<Listener>();
  set.add(listener);
  listeners.set(taskPath, set);

  const cached = cache.get(taskPath);
  if (cached !== undefined) {
    try {
      listener(cached);
    } catch {
      // ignore
    }
  }

  const staleMs = opts?.staleMs ?? DEFAULT_STALE_MS;
  if (!pending.has(taskPath) && (cached === undefined || isStale(cached, staleMs))) {
    refreshPrChanges(taskPath, { force: true }).catch(() => {});
  }

  return () => {
    const taskListeners = listeners.get(taskPath);
    if (taskListeners) {
      taskListeners.delete(listener);
      if (taskListeners.size === 0) {
        listeners.delete(taskPath);
      }
    }
  };
}
