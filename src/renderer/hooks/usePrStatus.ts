import { useEffect, useState } from 'react';
import { subscribeToPrStatus, refreshPrStatus } from '../lib/prStatusStore';
import type { PrStatus } from '../lib/prStatus';

export function usePrStatus(taskPath?: string) {
  const [pr, setPr] = useState<PrStatus | null>(null);

  const refresh = async () => {
    if (!taskPath) return;
    const result = await refreshPrStatus(taskPath);
    setPr(result);
  };

  useEffect(() => {
    if (!taskPath) {
      setTimeout(() => setPr(null), 0);
      return;
    }

    setTimeout(() => setPr(null), 0); // Clear stale data before subscribing to new task
    const unsubscribe = subscribeToPrStatus(taskPath, setPr);
    refreshPrStatus(taskPath).catch(() => {});
    return unsubscribe;
  }, [taskPath]);

  return { pr, refresh };
}
