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
  selectedTraceId: string | null;
  searchTraceId: string;
  maxTraces: number;
};

type Listener = () => void;

const MAX_TRACES = 50;
const MAX_ATTEMPTS_PER_TRACE = 100;

type TraceStoreState = {
  traces: TraceSession[];
  selectedTraceId: string | null;
  searchTraceId: string;
};

let state: TraceStoreState = {
  traces: [],
  selectedTraceId: null,
  searchTraceId: "",
};

let snapshot: TraceStoreSnapshot = {
  traces: state.traces,
  selectedTraceId: state.selectedTraceId,
  searchTraceId: state.searchTraceId,
  maxTraces: MAX_TRACES,
};

const listeners = new Set<Listener>();

function emit() {
  for (const listener of listeners) listener();
}

function setState(next: TraceStoreState) {
  state = next;
  snapshot = {
    traces: state.traces,
    selectedTraceId: state.selectedTraceId,
    searchTraceId: state.searchTraceId,
    maxTraces: MAX_TRACES,
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

function selectFallbackIfNeeded(nextTraces: TraceSession[]) {
  const selected = state.selectedTraceId;
  if (!selected) return state.selectedTraceId;
  const stillExists = nextTraces.some((t) => t.trace_id === selected);
  return stillExists ? selected : (nextTraces[0]?.trace_id ?? null);
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
    setState({
      ...state,
      traces: nextTraces,
      selectedTraceId: state.selectedTraceId ?? created.trace_id,
    });
    return;
  }

  const existing = state.traces[idx];
  const nextRequestedModel = payload.requested_model ?? existing.requested_model ?? null;
  const updated: TraceSession = {
    ...existing,
    cli_key: payload.cli_key,
    method: payload.method,
    path: payload.path,
    query: payload.query ?? null,
    requested_model: nextRequestedModel,
    last_seen_ms: now,
  };

  const nextTraces = state.traces.slice();
  nextTraces[idx] = updated;
  moveTraceToFront(nextTraces, updated.trace_id);

  setState({
    ...state,
    traces: nextTraces.slice(0, MAX_TRACES),
    selectedTraceId: selectFallbackIfNeeded(nextTraces),
  });
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
    setState({
      ...state,
      traces: nextTraces,
      selectedTraceId: state.selectedTraceId ?? created.trace_id,
    });
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

  setState({
    ...state,
    traces: nextTraces.slice(0, MAX_TRACES),
    selectedTraceId: selectFallbackIfNeeded(nextTraces),
  });
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
    setState({
      ...state,
      traces: nextTraces,
      selectedTraceId: state.selectedTraceId ?? created.trace_id,
    });
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

  setState({
    ...state,
    traces: nextTraces.slice(0, MAX_TRACES),
    selectedTraceId: selectFallbackIfNeeded(nextTraces),
  });
}

export function setSelectedTraceId(traceId: string | null) {
  if (traceId === state.selectedTraceId) return;
  setState({ ...state, selectedTraceId: traceId });
}

export function setSearchTraceId(value: string) {
  if (value === state.searchTraceId) return;
  setState({ ...state, searchTraceId: value });
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
