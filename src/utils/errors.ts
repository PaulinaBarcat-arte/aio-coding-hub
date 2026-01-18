export function formatUnknownError(err: unknown) {
  if (typeof err === "string") return err;
  if (err instanceof Error && err.message) return err.message;
  if (err && typeof err === "object") {
    const maybeMessage = (err as { message?: unknown }).message;
    if (typeof maybeMessage === "string" && maybeMessage.trim()) return maybeMessage;
    try {
      return JSON.stringify(err);
    } catch {
      // ignore
    }
  }
  try {
    return String(err);
  } catch {
    return "未知错误";
  }
}

export function parseErrorCodeMessage(raw: string): {
  error_code: string | null;
  message: string;
} {
  const trimmed = raw.trim();
  const msg = trimmed.replace(/^Error:\s*/i, "").trim();
  if (!msg) return { error_code: null, message: "未知错误" };

  const match = /^([A-Z][A-Z0-9_]*):\s*(.*)$/.exec(msg);
  if (!match) return { error_code: null, message: msg };
  const code = match[1] || null;
  const rest = (match[2] ?? "").trim();
  return { error_code: code, message: rest || msg };
}

export function compactWhitespace(text: string) {
  return text.replace(/\s+/g, " ").trim();
}

export function normalizeErrorWithCode(err: unknown): {
  raw: string;
  error_code: string | null;
  message: string;
} {
  const raw = formatUnknownError(err);
  const { error_code, message } = parseErrorCodeMessage(raw);
  return { raw, error_code, message: compactWhitespace(message) };
}

export function formatActionFailureToast(
  action: string,
  err: unknown
): {
  raw: string;
  error_code: string | null;
  message: string;
  toast: string;
} {
  const normalized = normalizeErrorWithCode(err);
  const codeLabel = normalized.error_code ? `（code ${normalized.error_code}）` : "";
  return {
    ...normalized,
    toast: `${action}失败${codeLabel}：${normalized.message}`,
  };
}
