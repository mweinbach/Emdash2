import { useCallback, useEffect, useState } from 'react';

type GithubUser = any;

type GithubCache = {
  installed: boolean;
  authenticated: boolean;
  user: GithubUser | null;
} | null;

let cachedGithubStatus: GithubCache = null;
type CacheListener = (cache: GithubCache) => void;
const cacheListeners = new Set<CacheListener>();

const notifyCacheListeners = (cache: GithubCache) => {
  cacheListeners.forEach((listener) => {
    try {
      listener(cache);
    } catch {}
  });
};

const waitForRuntimeReady = (timeoutMs = 1500) =>
  new Promise<void>((resolve) => {
    if (typeof window === 'undefined') {
      resolve();
      return;
    }
    const start = Date.now();
    const tick = () => {
      const api: any = (window as any).desktopAPI;
      const runtime = api?.__runtime;
      if (runtime === 'web' || api?.__runtimeReady) {
        resolve();
        return;
      }
      if (Date.now() - start >= timeoutMs) {
        resolve();
        return;
      }
      setTimeout(tick, 50);
    };
    tick();
  });

export function useGithubAuth() {
  const [installed, setInstalled] = useState<boolean>(() => cachedGithubStatus?.installed ?? true);
  const [authenticated, setAuthenticated] = useState<boolean>(
    () => cachedGithubStatus?.authenticated ?? false
  );
  const [user, setUser] = useState<GithubUser | null>(() => cachedGithubStatus?.user ?? null);
  const [isLoading, setIsLoading] = useState<boolean>(false);
  const [isInitialized, setIsInitialized] = useState<boolean>(() => !!cachedGithubStatus);

  const applyCache = useCallback((next: GithubCache) => {
    if (!next) return;
    setInstalled(next.installed);
    setAuthenticated(next.authenticated);
    setUser(next.user);
    setIsInitialized(true);
  }, []);

  const syncCache = useCallback(
    (next: { installed: boolean; authenticated: boolean; user: GithubUser | null }) => {
      cachedGithubStatus = next;
      applyCache(next);
      notifyCacheListeners(next);
    },
    [applyCache]
  );

  const checkStatus = useCallback(async () => {
    try {
      const api: any = (window as any).desktopAPI;
      if (api?.__runtime === 'tauri' && !api?.__runtimeReady) {
        await waitForRuntimeReady();
      }
      if (!api?.githubGetStatus) {
        throw new Error('GitHub status API unavailable');
      }
      const status = await api.githubGetStatus();
      const normalized = {
        installed: !!status?.installed,
        authenticated: !!status?.authenticated,
        user: status?.user || null,
      };
      syncCache(normalized);
      return status;
    } catch (e) {
      const fallback = { installed: false, authenticated: false, user: null };
      syncCache(fallback);
      return fallback;
    }
  }, [syncCache]);

  const login = useCallback(async () => {
    setIsLoading(true);
    try {
      const result = await window.desktopAPI.githubAuth();
      // Device Flow returns device code info, not immediate success
      // The modal will handle the actual authentication
      return result;
    } finally {
      setIsLoading(false);
    }
  }, [syncCache]);

  const logout = useCallback(async () => {
    try {
      await window.desktopAPI.githubLogout();
    } finally {
      syncCache({
        installed: cachedGithubStatus?.installed ?? true,
        authenticated: false,
        user: null,
      });
    }
  }, [syncCache]);

  useEffect(() => {
    const listener: CacheListener = (next) => {
      applyCache(next);
    };
    cacheListeners.add(listener);

    // If we have cached status, mark as initialized but still refresh in background
    if (cachedGithubStatus) {
      setIsInitialized(true);
      // Still refresh in background to ensure we have the latest status
      void checkStatus();
      return () => {
        cacheListeners.delete(listener);
      };
    }
    // No cache - check status immediately
    void checkStatus();
    return () => {
      cacheListeners.delete(listener);
    };
  }, [checkStatus, applyCache]);

  return {
    installed,
    authenticated,
    user,
    isLoading,
    isInitialized,
    checkStatus,
    login,
    logout,
  };
}
