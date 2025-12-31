type GitStatusChange = {
  path: string;
  status: string;
  additions: number;
  deletions: number;
  isStaged: boolean;
  diff?: string;
};

export type GitStatusResult = {
  success: boolean;
  changes?: GitStatusChange[];
  error?: string;
};

type Listener = (status: GitStatusResult | null) => void;
type SubscriptionOptions = {
  intervalMs?: number;
};

const DEFAULT_POLL_MS = 10_000;
const cache = new Map<string, GitStatusResult | null>();
const fingerprints = new Map<string, string>();
const listeners = new Map<string, Map<Listener, number>>();
const pending = new Map<string, Promise<GitStatusResult | null>>();
const pollTimers = new Map<string, ReturnType<typeof setTimeout>>();
const pollIntervals = new Map<string, number>();
const lastFetchedAt = new Map<string, number>();

const computeFingerprint = (status: GitStatusResult | null): string => {
  if (!status) return 'null';
  if (!status.success) return `err:${status.error || ''}`;
  const changes = status.changes || [];
  if (changes.length === 0) return 'ok:0';
  return changes
    .map(
      (c) =>
        `${c.path}|${c.status}|${Number(c.additions || 0)}|${Number(c.deletions || 0)}|${
          c.isStaged ? 1 : 0
        }`
    )
    .join(';');
};

const getMinInterval = (taskPath: string): number | null => {
  const subs = listeners.get(taskPath);
  if (!subs || subs.size === 0) return null;
  let min = Number.POSITIVE_INFINITY;
  for (const interval of subs.values()) {
    if (typeof interval !== 'number' || interval <= 0) continue;
    if (interval < min) min = interval;
  }
  return Number.isFinite(min) ? min : null;
};

const stopPolling = (taskPath: string) => {
  const timer = pollTimers.get(taskPath);
  if (timer) {
    clearTimeout(timer);
  }
  pollTimers.delete(taskPath);
  pollIntervals.delete(taskPath);
};

const schedulePoll = (taskPath: string) => {
  const interval = getMinInterval(taskPath);
  if (!interval) {
    stopPolling(taskPath);
    return;
  }
  const currentInterval = pollIntervals.get(taskPath);
  if (currentInterval === interval && pollTimers.has(taskPath)) {
    return;
  }
  stopPolling(taskPath);
  pollIntervals.set(taskPath, interval);
  const jitter = Math.min(1000, Math.max(0, interval * 0.1));
  const delay = interval + Math.floor(Math.random() * jitter);
  const timer = setTimeout(async () => {
    pollTimers.delete(taskPath);
    await refreshGitStatus(taskPath, { minIntervalMs: interval });
    if (listeners.get(taskPath)?.size) {
      schedulePoll(taskPath);
    } else {
      stopPolling(taskPath);
    }
  }, delay);
  pollTimers.set(taskPath, timer);
};

const notify = (taskPath: string, status: GitStatusResult | null) => {
  const fingerprint = computeFingerprint(status);
  const prevFingerprint = fingerprints.get(taskPath);
  cache.set(taskPath, status);
  if (fingerprint === prevFingerprint) return;
  fingerprints.set(taskPath, fingerprint);
  const subs = listeners.get(taskPath);
  if (!subs) return;
  for (const fn of subs.keys()) {
    try {
      fn(status);
    } catch {}
  }
};

const fetchGitStatus = async (taskPath: string): Promise<GitStatusResult | null> => {
  try {
    const api: any = (window as any).desktopAPI;
    if (!api?.getGitStatus) {
      return { success: false, error: 'Git status unavailable' };
    }
    const res = await api.getGitStatus(taskPath);
    if (!res || typeof res.success !== 'boolean') {
      return { success: false, error: 'Invalid git status response' };
    }
    return res as GitStatusResult;
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : String(error),
    };
  }
};

export const getCachedGitStatus = (taskPath: string) => cache.get(taskPath) ?? null;

export async function refreshGitStatus(
  taskPath: string,
  options: { force?: boolean; minIntervalMs?: number } = {}
): Promise<GitStatusResult | null> {
  if (!taskPath) return null;
  const now = Date.now();
  const minInterval = options.minIntervalMs ?? 0;
  const last = lastFetchedAt.get(taskPath);
  if (!options.force && last && minInterval > 0 && now - last < minInterval) {
    return cache.get(taskPath) ?? null;
  }

  const inFlight = pending.get(taskPath);
  if (inFlight) return inFlight;

  const promise = (async () => {
    const result = await fetchGitStatus(taskPath);
    lastFetchedAt.set(taskPath, Date.now());
    notify(taskPath, result);
    return result;
  })();
  pending.set(taskPath, promise);

  try {
    return await promise;
  } finally {
    pending.delete(taskPath);
  }
}

export function subscribeGitStatus(
  taskPath: string,
  listener: Listener,
  options: SubscriptionOptions = {}
): () => void {
  if (!taskPath) return () => {};
  const intervalMs = options.intervalMs ?? DEFAULT_POLL_MS;
  const subs = listeners.get(taskPath) || new Map<Listener, number>();
  subs.set(listener, intervalMs);
  listeners.set(taskPath, subs);

  const cached = cache.get(taskPath) ?? null;
  const hasCache = cache.has(taskPath);
  if (hasCache) {
    try {
      listener(cached);
    } catch {}
  }

  if (!hasCache || cached === null || cached?.success === false) {
    void refreshGitStatus(taskPath, { minIntervalMs: intervalMs });
  }

  schedulePoll(taskPath);

  return () => {
    const current = listeners.get(taskPath);
    if (current) {
      current.delete(listener);
      if (current.size === 0) {
        listeners.delete(taskPath);
        stopPolling(taskPath);
      } else {
        schedulePoll(taskPath);
      }
    }
  };
}
