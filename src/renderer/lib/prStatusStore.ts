import type { PrStatus } from './prStatus';

type Listener = (pr: PrStatus | null) => void;

const cache = new Map<string, PrStatus | null>();
const lastFetched = new Map<string, number>();
const listeners = new Map<string, Set<Listener>>();
const pending = new Map<string, Promise<PrStatus | null>>();
const DEFAULT_STALE_MS = 60_000;

async function fetchPrStatus(taskPath: string): Promise<PrStatus | null> {
  try {
    const res = await window.desktopAPI.getPrStatus({ taskPath });
    if (res?.success && res.pr) {
      lastFetched.set(taskPath, Date.now());
      return res.pr as PrStatus;
    }
    lastFetched.set(taskPath, Date.now());
    return null;
  } catch (error) {
    return null;
  }
}

function isStale(taskPath: string, cached: PrStatus | null | undefined, staleMs: number) {
  if (!cached) return true;
  const ts = lastFetched.get(taskPath);
  if (!ts) return true;
  return Date.now() - ts > staleMs;
}

export async function refreshPrStatus(taskPath: string): Promise<PrStatus | null> {
  // Deduplicate concurrent requests
  const inFlight = pending.get(taskPath);
  if (inFlight) return inFlight;

  const promise = fetchPrStatus(taskPath);
  pending.set(taskPath, promise);

  try {
    const pr = await promise;
    cache.set(taskPath, pr);

    // Notify all listeners
    const taskListeners = listeners.get(taskPath);
    if (taskListeners) {
      for (const listener of taskListeners) {
        try {
          listener(pr);
        } catch {}
      }
    }

    return pr;
  } finally {
    pending.delete(taskPath);
  }
}

export function subscribeToPrStatus(taskPath: string, listener: Listener): () => void {
  const set = listeners.get(taskPath) || new Set<Listener>();
  set.add(listener);
  listeners.set(taskPath, set);

  // Emit current cached value if available
  const cached = cache.get(taskPath);
  if (cached !== undefined) {
    try {
      listener(cached);
    } catch {}
  }

  // Trigger fetch if not cached or if data is stale
  if (!pending.has(taskPath) && (cached === undefined || isStale(taskPath, cached, DEFAULT_STALE_MS))) {
    refreshPrStatus(taskPath).catch(() => {});
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
