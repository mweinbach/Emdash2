import { useEffect, useState } from 'react';
import type { GitStatusResult } from '../lib/gitStatusStore';
import { subscribeGitStatus } from '../lib/gitStatusStore';

export interface TaskChange {
  path: string;
  status: string;
  additions: number;
  deletions: number;
  diff?: string;
}

export interface TaskChanges {
  taskId: string;
  changes: TaskChange[];
  totalAdditions: number;
  totalDeletions: number;
  isLoading: boolean;
  error?: string;
}

export function useTaskChanges(taskPath: string, taskId: string) {
  const [changes, setChanges] = useState<TaskChanges>({
    taskId,
    changes: [],
    totalAdditions: 0,
    totalDeletions: 0,
    isLoading: true,
  });

  useEffect(() => {
    if (!taskPath) {
      setChanges({
        taskId,
        changes: [],
        totalAdditions: 0,
        totalDeletions: 0,
        isLoading: false,
        error: 'Missing task path',
      });
      return;
    }

    let cancelled = false;
    setChanges((prev) => ({ ...prev, taskId, isLoading: true, error: undefined }));

    const handleStatus = (result: GitStatusResult | null) => {
      if (cancelled) return;
      if (result?.success) {
        const filtered = (result.changes || []).filter(
          (c: { path: string }) => !c.path.startsWith('.emdash/') && c.path !== 'PLANNING.md'
        );
        const totalAdditions = filtered.reduce((sum, change) => sum + (change.additions || 0), 0);
        const totalDeletions = filtered.reduce((sum, change) => sum + (change.deletions || 0), 0);
        setChanges({
          taskId,
          changes: filtered,
          totalAdditions,
          totalDeletions,
          isLoading: false,
        });
      } else {
        setChanges({
          taskId,
          changes: [],
          totalAdditions: 0,
          totalDeletions: 0,
          isLoading: false,
          error: result?.error || 'Failed to fetch changes',
        });
      }
    };

    const unsubscribe = subscribeGitStatus(taskPath, handleStatus, { intervalMs: 10000 });

    return () => {
      cancelled = true;
      unsubscribe();
    };
  }, [taskPath, taskId]);

  return {
    ...changes,
  };
}
