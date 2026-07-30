import { useState, useEffect, useCallback } from 'react';
import { useSSE } from './useSSE';
import { getEstopStatus } from '@/lib/api';
import type { EstopStatus } from '@/types/api';

const POLL_INTERVAL_MS = 30_000;

/**
 * Tracks live emergency-stop status for the whole app: an initial fetch plus
 * a periodic poll fallback, kept fresh in real time via `estop_status` SSE
 * events. Returns `null` while loading or when estop is disabled/unreachable.
 */
export function useEstopStatus() {
  const [status, setStatus] = useState<EstopStatus | null>(null);
  const { events } = useSSE({ filterTypes: ['estop_status'] });

  const refresh = useCallback(async () => {
    try {
      const data = await getEstopStatus();
      setStatus(data);
    } catch {
      // Disabled, unauthenticated, or gateway unreachable — no banner to show.
      setStatus(null);
    }
  }, []);

  useEffect(() => {
    refresh();
    const id = setInterval(refresh, POLL_INTERVAL_MS);
    return () => clearInterval(id);
  }, [refresh]);

  useEffect(() => {
    const last = events[events.length - 1];
    if (!last) return;
    setStatus((prev) => ({
      enabled: true,
      is_engaged: Boolean(last.is_engaged),
      kill_all: Boolean(last.kill_all),
      network_kill: Boolean(last.network_kill),
      blocked_domains: Array.isArray(last.blocked_domains)
        ? (last.blocked_domains as string[])
        : prev?.blocked_domains ?? [],
      frozen_tools: Array.isArray(last.frozen_tools)
        ? (last.frozen_tools as string[])
        : prev?.frozen_tools ?? [],
      updated_at: (last.updated_at as string | undefined) ?? prev?.updated_at ?? null,
      require_otp_to_resume: prev?.require_otp_to_resume ?? false,
    }));
  }, [events]);

  return { status, refresh };
}
