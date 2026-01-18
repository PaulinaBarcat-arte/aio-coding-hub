import { useSyncExternalStore } from "react";

export type ConsoleLogLevel = "info" | "warn" | "error";

export type ConsoleLogMeta = {
  trace_id?: string;
  cli_key?: string;
  providers?: string[];
  error_code?: string;
};

export type ConsoleLogEntry = {
  id: string;
  ts: number;
  level: ConsoleLogLevel;
  title: string;
  details?: string;
  meta?: ConsoleLogMeta;
};

type Listener = () => void;

let entries: ConsoleLogEntry[] = [];
const listeners = new Set<Listener>();

function emit() {
  for (const listener of listeners) listener();
}

function randomId() {
  return typeof crypto !== "undefined" && "randomUUID" in crypto
    ? crypto.randomUUID()
    : `${Date.now()}-${Math.random().toString(16).slice(2)}`;
}

function isSensitiveKey(key: string): boolean {
  const k = key.toLowerCase();
  return (
    k.includes("api_key") ||
    k.includes("apikey") ||
    k.includes("authorization") ||
    k === "token" ||
    k.endsWith("_token") ||
    k.endsWith("token")
  );
}

function sanitizeDetails(value: unknown, seen: WeakSet<object>, depth: number): unknown {
  if (value === null) return value;
  if (depth > 6) return "[Truncated]";

  if (typeof value !== "object") return value;
  if (seen.has(value)) return "[Circular]";
  seen.add(value);

  if (Array.isArray(value)) {
    return value.map((item) => sanitizeDetails(item, seen, depth + 1));
  }

  const input = value as Record<string, unknown>;
  const out: Record<string, unknown> = {};

  for (const [k, v] of Object.entries(input)) {
    out[k] = isSensitiveKey(k) ? "[REDACTED]" : sanitizeDetails(v, seen, depth + 1);
  }

  return out;
}

function toDetails(value: unknown): string | undefined {
  if (value === undefined) return undefined;
  if (typeof value === "string") return value;
  try {
    const sanitized = sanitizeDetails(value, new WeakSet(), 0);
    return JSON.stringify(sanitized, null, 2);
  } catch {
    return String(value);
  }
}

function asRecord(value: unknown): Record<string, unknown> | null {
  if (!value || typeof value !== "object") return null;
  if (Array.isArray(value)) return null;
  return value as Record<string, unknown>;
}

function normalizeString(value: unknown): string | undefined {
  if (typeof value !== "string") return undefined;
  const trimmed = value.trim();
  return trimmed ? trimmed : undefined;
}

function uniqueStrings(values: string[]): string[] {
  const set = new Set<string>();
  for (const value of values) {
    if (!value) continue;
    set.add(value);
  }
  return Array.from(set);
}

function extractMeta(details: unknown): ConsoleLogMeta | undefined {
  const record = asRecord(details);
  if (!record) return undefined;

  const traceId = normalizeString(record.trace_id ?? record.traceId);
  const cliKey = normalizeString(record.cli_key ?? record.cliKey ?? record.cli);
  const errorCode = normalizeString(record.error_code ?? record.errorCode);

  const providers: string[] = [];

  const directProvider = normalizeString(record.provider_name ?? record.providerName);
  if (directProvider) providers.push(directProvider);

  const attempts = record.attempts;
  if (Array.isArray(attempts)) {
    for (const attempt of attempts) {
      const attemptRecord = asRecord(attempt);
      if (!attemptRecord) continue;
      const attemptProvider = normalizeString(
        attemptRecord.provider_name ?? attemptRecord.providerName
      );
      if (attemptProvider) providers.push(attemptProvider);
    }
  }

  const explicitProviders = record.providers;
  if (Array.isArray(explicitProviders)) {
    for (const p of explicitProviders) {
      const name = normalizeString(p);
      if (name) providers.push(name);
    }
  }

  const meta: ConsoleLogMeta = {};
  if (traceId) meta.trace_id = traceId;
  if (cliKey) meta.cli_key = cliKey;
  if (errorCode) meta.error_code = errorCode;

  const uniqueProviders = uniqueStrings(providers).slice(0, 12);
  if (uniqueProviders.length > 0) meta.providers = uniqueProviders;

  return Object.keys(meta).length > 0 ? meta : undefined;
}

export function logToConsole(level: ConsoleLogLevel, title: string, details?: unknown) {
  const entry: ConsoleLogEntry = {
    id: randomId(),
    ts: Date.now(),
    level,
    title,
    details: toDetails(details),
    meta: extractMeta(details),
  };

  entries = [entry, ...entries].slice(0, 500);
  emit();
}

export function clearConsoleLogs() {
  entries = [];
  emit();
}

export function useConsoleLogs() {
  return useSyncExternalStore(
    (listener) => {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
    () => entries,
    () => entries
  );
}
