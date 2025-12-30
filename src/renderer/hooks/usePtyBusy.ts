import { useEffect, useRef, useState } from 'react';
import { classifyActivity } from '../lib/activityClassifier';
import { BUSY_HOLD_MS, CLEAR_BUSY_MS } from '../lib/activityConstants';

export function usePtyBusy(ptyId?: string, provider?: string) {
  const [busy, setBusy] = useState(false);
  const busySinceRef = useRef<number | null>(null);
  const clearTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const busyRef = useRef(false);

  const clearTimer = () => {
    if (clearTimerRef.current) {
      clearTimeout(clearTimerRef.current);
      clearTimerRef.current = null;
    }
  };

  const setBusyState = (next: boolean) => {
    if (next) {
      clearTimer();
      busySinceRef.current = Date.now();
      setBusy((prev) => {
        if (prev) return prev;
        return true;
      });
      busyRef.current = true;
      return;
    }

    const started = busySinceRef.current || 0;
    const elapsed = started ? Date.now() - started : BUSY_HOLD_MS;
    const remaining = elapsed < BUSY_HOLD_MS ? BUSY_HOLD_MS - elapsed : 0;

    const clearNow = () => {
      clearTimer();
      busySinceRef.current = null;
      busyRef.current = false;
      setBusy(false);
    };

    if (remaining > 0) {
      clearTimer();
      clearTimerRef.current = setTimeout(clearNow, remaining);
    } else {
      clearNow();
    }
  };

  const armSoftClear = () => {
    clearTimer();
    clearTimerRef.current = setTimeout(() => setBusyState(false), CLEAR_BUSY_MS);
  };

  useEffect(() => {
    if (!ptyId) {
      setBusy(false);
      busyRef.current = false;
      return;
    }

    const api: any = (window as any).electronAPI;
    if (!api?.onPtyData) {
      setBusy(false);
      return;
    }

    const offData = api.onPtyData(ptyId, (chunk: string) => {
      const signal = classifyActivity(provider, chunk || '');
      if (signal === 'busy') {
        setBusyState(true);
      } else if (signal === 'idle') {
        setBusyState(false);
      } else if (busyRef.current) {
        armSoftClear();
      }
    });

    const offExit = api?.onPtyExit?.(ptyId, () => {
      setBusyState(false);
    });

    return () => {
      clearTimer();
      try {
        offData?.();
      } catch {}
      try {
        offExit?.();
      } catch {}
    };
  }, [ptyId, provider]);

  return busy;
}
