import { useSyncExternalStore } from "react";
import type {
  GatewayAttemptEvent,
  GatewayRequestEvent,
  GatewayRequestStartEvent,
} from "./gatewayEvents";

export type TraceSession = {
  trace_id: string;
  cli_key: string;
  method: string;
  path: string;
  query: string | null;
  requested_model?: string | null;
  first_seen_ms: number;
  last_seen_ms: number;
  attempts: GatewayAttemptEvent[];
  summary?: GatewayRequestEvent;
};

export type TraceStoreSnapshot = {
  traces: TraceSession[];
};

type Listener = () => void;

const MAX_TRACES = 50;
const MAX_ATTEMPTS_PER_TRACE = 100;

type TraceStoreState = {
  traces: TraceSession[];
};

let state: TraceStoreState = {
  traces: [],
};

let snapshot: TraceStoreSnapshot = {
  traces: state.traces,
};

const listeners = new Set<Listener>();

function emit() {
  for (const listener of listeners) listener();
}

function setState(next: TraceStoreState) {
  state = next;
  snapshot = {
    traces: state.traces,
  };
  emit();
}

function findTraceIndex(traceId: string): number {
  return state.traces.findIndex((trace) => trace.trace_id === traceId);
}

function upsertAttempt(
  attempts: GatewayAttemptEvent[],
  payload: GatewayAttemptEvent
): GatewayAttemptEvent[] {
  const next = attempts.filter((a) => a.attempt_index !== payload.attempt_index);
  next.push(payload);
  next.sort((a, b) => a.attempt_index - b.attempt_index);
  return next.slice(-MAX_ATTEMPTS_PER_TRACE);
}

function moveTraceToFront(nextTraces: TraceSession[], traceId: string) {
  const index = nextTraces.findIndex((t) => t.trace_id === traceId);
  if (index <= 0) return nextTraces;
  const trace = nextTraces[index];
  nextTraces.splice(index, 1);
  nextTraces.unshift(trace);
  return nextTraces;
}

export function ingestTraceStart(payload: GatewayRequestStartEvent) {
  if (!payload?.trace_id) return;

  const now = Date.now();
  const idx = findTraceIndex(payload.trace_id);

  if (idx === -1) {
    const created: TraceSession = {
      trace_id: payload.trace_id,
      cli_key: payload.cli_key,
      method: payload.method,
      path: payload.path,
      query: payload.query ?? null,
      requested_model: payload.requested_model ?? null,
      first_seen_ms: now,
      last_seen_ms: now,
      attempts: [],
    };

    const nextTraces = [created, ...state.traces].slice(0, MAX_TRACES);
    setState({ traces: nextTraces });
    return;
  }

  const existing = state.traces[idx];
  const nextRequestedModel = payload.requested_model ?? existing.requested_model ?? null;
  const shouldReset = Boolean(existing.summary);
  const updated: TraceSession = {
    ...existing,
    cli_key: payload.cli_key,
    method: payload.method,
    path: payload.path,
    query: payload.query ?? null,
    requested_model: nextRequestedModel,
    last_seen_ms: now,
    ...(shouldReset ? { first_seen_ms: now, attempts: [], summary: undefined } : {}),
  };

  const nextTraces = state.traces.slice();
  nextTraces[idx] = updated;
  moveTraceToFront(nextTraces, updated.trace_id);

  setState({ traces: nextTraces.slice(0, MAX_TRACES) });
}

export function ingestTraceAttempt(payload: GatewayAttemptEvent) {
  if (!payload?.trace_id) return;

  const now = Date.now();
  const idx = findTraceIndex(payload.trace_id);

  if (idx === -1) {
    const created: TraceSession = {
      trace_id: payload.trace_id,
      cli_key: payload.cli_key,
      method: payload.method,
      path: payload.path,
      query: payload.query ?? null,
      requested_model: null,
      first_seen_ms: now,
      last_seen_ms: now,
      attempts: [payload],
    };

    const nextTraces = [created, ...state.traces].slice(0, MAX_TRACES);
    setState({ traces: nextTraces });
    return;
  }

  const existing = state.traces[idx];
  const updated: TraceSession = {
    ...existing,
    cli_key: payload.cli_key,
    method: payload.method,
    path: payload.path,
    query: payload.query ?? null,
    last_seen_ms: now,
    attempts: upsertAttempt(existing.attempts, payload),
  };

  const nextTraces = state.traces.slice();
  nextTraces[idx] = updated;
  moveTraceToFront(nextTraces, updated.trace_id);

  setState({ traces: nextTraces.slice(0, MAX_TRACES) });
}

export function ingestTraceRequest(payload: GatewayRequestEvent) {
  if (!payload?.trace_id) return;

  const now = Date.now();
  const idx = findTraceIndex(payload.trace_id);

  if (idx === -1) {
    const created: TraceSession = {
      trace_id: payload.trace_id,
      cli_key: payload.cli_key,
      method: payload.method,
      path: payload.path,
      query: payload.query ?? null,
      requested_model: null,
      first_seen_ms: now,
      last_seen_ms: now,
      attempts: [],
      summary: payload,
    };

    const nextTraces = [created, ...state.traces].slice(0, MAX_TRACES);
    setState({ traces: nextTraces });
    return;
  }

  const existing = state.traces[idx];
  const updated: TraceSession = {
    ...existing,
    cli_key: payload.cli_key,
    method: payload.method,
    path: payload.path,
    query: payload.query ?? null,
    last_seen_ms: now,
    summary: payload,
  };

  const nextTraces = state.traces.slice();
  nextTraces[idx] = updated;
  moveTraceToFront(nextTraces, updated.trace_id);

  setState({ traces: nextTraces.slice(0, MAX_TRACES) });
}

export function useTraceStore(): TraceStoreSnapshot {
  return useSyncExternalStore(
    (listener) => {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
    () => snapshot,
    () => snapshot
  );
}
