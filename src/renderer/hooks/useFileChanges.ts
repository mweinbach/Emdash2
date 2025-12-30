import { useEffect, useRef, useState } from 'react';
import type { GitStatusResult } from '../lib/gitStatusStore';
import { refreshGitStatus, subscribeGitStatus } from '../lib/gitStatusStore';

export interface FileChange {
  path: string;
  status: 'added' | 'modified' | 'deleted' | 'renamed';
  additions: number;
  deletions: number;
  isStaged: boolean;
  diff?: string;
}

export function useFileChanges(taskPath: string) {
  const [fileChanges, setFileChanges] = useState<FileChange[]>([]);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const awaitingRefresh = useRef(false);

  useEffect(() => {
    if (!taskPath) {
      setFileChanges([]);
      setIsLoading(false);
      setError('Missing task path');
      return;
    }

    let cancelled = false;
    setIsLoading(true);
    setError(null);

    const handleStatus = (result: GitStatusResult | null) => {
      if (cancelled) return;
      if (result?.success && result.changes && result.changes.length > 0) {
        const changes: FileChange[] = result.changes
          .map((change) => ({
            path: change.path,
            status: change.status as 'added' | 'modified' | 'deleted' | 'renamed',
            additions: change.additions || 0,
            deletions: change.deletions || 0,
            isStaged: change.isStaged || false,
            diff: change.diff,
          }))
          .filter((c) => !c.path.startsWith('.emdash/') && c.path !== 'PLANNING.md');
        setFileChanges(changes);
        setError(null);
      } else if (result?.success) {
        setFileChanges([]);
        setError(null);
      } else {
        setFileChanges([]);
        setError(result?.error || 'Failed to load file changes');
      }
      if (awaitingRefresh.current) {
        awaitingRefresh.current = false;
      }
      setIsLoading(false);
    };

    const unsubscribe = subscribeGitStatus(taskPath, handleStatus, { intervalMs: 5000 });

    return () => {
      cancelled = true;
      unsubscribe();
    };
  }, [taskPath]);

  const refreshChanges = async () => {
    if (!taskPath) return;
    awaitingRefresh.current = true;
    setIsLoading(true);
    await refreshGitStatus(taskPath, { force: true });
  };

  return {
    fileChanges,
    isLoading,
    error,
    refreshChanges,
  };
}
