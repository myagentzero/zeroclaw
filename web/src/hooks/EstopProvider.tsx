import {
  createContext,
  useContext,
  useEffect,
  useRef,
  useState,
  useCallback,
  useMemo,
  type ReactNode,
} from 'react';
import { useSharedSSE } from './SSEProvider';
import { getEstopStatus } from '@/lib/api';
import type { EstopStatus } from '@/types/api';

interface EstopContextValue {
  status: EstopStatus | null;
  /** True until the first fetch (success or failure) completes. */
  loading: boolean;
  /** True if the server reports emergency stop is disabled. */
  disabled: boolean;
  /** Set when the last fetch failed for a reason other than "disabled". */
  loadError: string | null;
  refresh: () => Promise<void>;
  setStatus: (status: EstopStatus | null) => void;
}

const EstopContext = createContext<EstopContextValue | null>(null);

/**
 * Owns the single `/api/estop` REST fetch for the whole app, mounted once
 * near the root. The header banner and the Estop page both read from this
 * context instead of each fetching status independently. Stays fresh via
 * the shared `estop_status` SSE feed, re-fetching only on (re)connect since
 * a dropped connection may have missed an engage/resume broadcast.
 */
export function EstopProvider({ children }: { children: ReactNode }) {
  const [status, setStatus] = useState<EstopStatus | null>(null);
  const [loading, setLoading] = useState(true);
  const [disabled, setDisabled] = useState(false);
  const [loadError, setLoadError] = useState<string | null>(null);
  const { events, status: sseStatus } = useSharedSSE(['estop_status']);

  const refresh = useCallback(async () => {
    try {
      const data = await getEstopStatus();
      setStatus(data);
      setDisabled(false);
      setLoadError(null);
    } catch (err: unknown) {
      const message = err instanceof Error ? err.message : 'Failed to load estop status';
      if (message.toLowerCase().includes('disabled')) {
        setDisabled(true);
      } else {
        setLoadError(message);
      }
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    refresh();
  }, [refresh]);

  const sawDisconnect = useRef(false);
  useEffect(() => {
    if (sseStatus !== 'connected') {
      sawDisconnect.current = true;
    } else if (sawDisconnect.current) {
      sawDisconnect.current = false;
      refresh();
    }
  }, [sseStatus, refresh]);

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

  const value = useMemo(
    () => ({ status, loading, disabled, loadError, refresh, setStatus }),
    [status, loading, disabled, loadError, refresh],
  );

  return <EstopContext.Provider value={value}>{children}</EstopContext.Provider>;
}

/** Reads the shared emergency-stop status. Must be used within `EstopProvider`. */
export function useEstopStatus(): EstopContextValue {
  const ctx = useContext(EstopContext);
  if (!ctx) {
    throw new Error('useEstopStatus must be used within an EstopProvider');
  }
  return ctx;
}
