import {
  createContext,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from 'react';
import { SSEClient } from '../lib/sse';
import type { SSEEvent } from '../types/api';

export type SSEConnectionStatus = 'disconnected' | 'connecting' | 'connected';

const MAX_EVENTS = 500;

interface SSEContextValue {
  events: SSEEvent[];
  status: SSEConnectionStatus;
}

const SSEContext = createContext<SSEContextValue>({ events: [], status: 'disconnected' });

/**
 * Owns the single `/api/events` connection for the whole app. Mount once,
 * near the root, while authenticated — every consumer (header estop banner,
 * Mission Control, etc.) reads from this shared feed via `useSharedSSE`
 * instead of each opening its own redundant connection.
 */
export function SSEProvider({ children }: { children: ReactNode }) {
  const [status, setStatus] = useState<SSEConnectionStatus>('disconnected');
  const [events, setEvents] = useState<SSEEvent[]>([]);

  useEffect(() => {
    const client = new SSEClient();

    client.onConnect = () => setStatus('connected');
    client.onError = () => setStatus('disconnected');
    client.onEvent = (event: SSEEvent) => {
      setEvents((prev) => {
        const next = [...prev, event];
        return next.length > MAX_EVENTS ? next.slice(next.length - MAX_EVENTS) : next;
      });
    };

    setStatus('connecting');
    client.connect();

    return () => {
      client.disconnect();
    };
  }, []);

  const value = useMemo(() => ({ events, status }), [events, status]);

  return <SSEContext.Provider value={value}>{children}</SSEContext.Provider>;
}

export interface UseSharedSSEResult {
  events: SSEEvent[];
  status: SSEConnectionStatus;
}

/**
 * Reads from the app-wide SSE feed owned by `SSEProvider`, optionally
 * filtered to specific event types. Does not open its own connection.
 */
export function useSharedSSE(filterTypes?: string[]): UseSharedSSEResult {
  const ctx = useContext(SSEContext);
  const filterRef = useRef(filterTypes);
  filterRef.current = filterTypes;

  const filtered = useMemo(() => {
    const types = filterRef.current;
    if (!types || types.length === 0) return ctx.events;
    return ctx.events.filter((event) => types.includes(event.type));
    // Re-filter whenever the underlying event list changes; the filter list
    // itself is read from a ref so callers can pass an inline array.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [ctx.events]);

  return { events: filtered, status: ctx.status };
}
