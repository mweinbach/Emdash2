import { useEffect, useState } from 'react';
import { activityStore } from '../lib/activityStore';

export function useTaskBusy(taskId?: string) {
  const [busy, setBusy] = useState(false);
  useEffect(() => {
    if (!taskId) {
      setTimeout(() => setBusy(false), 0);
      return;
    }
    return activityStore.subscribe(taskId, setBusy);
  }, [taskId]);
  return busy;
}
