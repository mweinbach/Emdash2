import { useCallback, useEffect, useState } from 'react';
import type { PrCommit, PrFile } from '../lib/prStatus';
import { refreshPrChanges, subscribeToPrChanges } from '../lib/prChangesStore';

type PrChangesState = {
  commits: PrCommit[];
  files: PrFile[];
  loading: boolean;
  error: string | null;
};

export function usePrChanges(taskPath?: string, enabled: boolean = true) {
  const [state, setState] = useState<PrChangesState>({
    commits: [],
    files: [],
    loading: false,
    error: null,
  });

  const refresh = useCallback(async () => {
    if (!taskPath) return;
    setState({ commits: [], files: [], loading: true, error: null });
    try {
      const data = await refreshPrChanges(taskPath, { force: true });
      setState({
        commits: data?.commits ?? [],
        files: data?.files ?? [],
        loading: false,
        error: null,
      });
    } catch (error: any) {
      setState((prev) => ({
        commits: prev.commits,
        files: prev.files,
        loading: false,
        error: error?.message || 'Failed to load pull request changes.',
      }));
    }
  }, [taskPath]);

  useEffect(() => {
    if (!enabled) return;
    if (!taskPath) {
      setState({ commits: [], files: [], loading: false, error: null });
      return;
    }

    setState((prev) => ({ ...prev, loading: true, error: null }));
    const unsubscribe = subscribeToPrChanges(taskPath, (data) => {
      setState({
        commits: data?.commits ?? [],
        files: data?.files ?? [],
        loading: false,
        error: null,
      });
    });
    void refresh();
    return unsubscribe;
  }, [enabled, refresh, taskPath]);

  return {
    commits: state.commits,
    files: state.files,
    loading: state.loading,
    error: state.error,
    refresh,
  };
}
