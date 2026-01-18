import {
  CLAUDE_VALIDATION_TEMPLATES,
  DEFAULT_CLAUDE_VALIDATION_TEMPLATE_KEY,
  type ClaudeValidationTemplate,
  type ClaudeValidationTemplateKey,
} from "../config/claudeValidationTemplates";
import {
  buildClaudeCliMetadataUserId,
  buildClaudeCliValidateHeaders,
  newUuidV4,
} from "../constants/claudeValidation";
import type { ClaudeModelValidationResult } from "./claudeModelValidation";

export {
  CLAUDE_VALIDATION_TEMPLATES,
  DEFAULT_CLAUDE_VALIDATION_TEMPLATE_KEY,
  type ClaudeValidationTemplate,
  type ClaudeValidationTemplateKey,
};

type ClaudeValidationExpect = {
  max_output_chars?: number;
  exact_output_chars?: number;
};

function isPlainObject(value: unknown): value is Record<string, unknown> {
  return Boolean(value) && typeof value === "object" && !Array.isArray(value);
}

function get<T>(obj: unknown, key: string): T | null {
  if (!isPlainObject(obj)) return null;
  return (obj as Record<string, unknown>)[key] as T;
}

const TOOL_SUPPORT_KEYWORDS_EN = [
  "bash",
  "file",
  "read",
  "write",
  "execute",
  "command",
  "shell",
] as const;

const TOOL_SUPPORT_KEYWORDS_ZH = ["编辑", "读取", "写入", "执行", "文件", "命令行"] as const;

const REVERSE_PROXY_KEYWORDS = ["zen", "warp", "kiro"] as const;

function listKeywordHits(text: string, keywords: readonly string[]) {
  const normalized = text.toLowerCase();
  const hits: string[] = [];
  for (const keyword of keywords) {
    if (!keyword) continue;
    if (normalized.includes(keyword.toLowerCase())) hits.push(keyword);
  }
  return [...new Set(hits)];
}

function listWordBoundaryHits(text: string, keywords: readonly string[]) {
  if (!text) return [];
  const hits: string[] = [];
  for (const keyword of keywords) {
    if (!keyword) continue;
    try {
      const re = new RegExp(`\\b${escapeRegExp(keyword)}\\b`, "i");
      if (re.test(text)) hits.push(keyword);
    } catch {
      // ignore
    }
  }
  return [...new Set(hits)];
}

function formatHitSummary(hits: string[], max = 6) {
  if (hits.length === 0) return "";
  const shown = hits.slice(0, max);
  const rest = hits.length - shown.length;
  return rest > 0 ? `${shown.join(", ")} +${rest}` : shown.join(", ");
}

function escapeRegExp(value: string) {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

function firstNonEmptyLine(text: string) {
  const lines = text.split(/\r?\n/);
  for (const line of lines) {
    const trimmed = line.trim();
    if (trimmed) return trimmed;
  }
  return "";
}

function truncateText(value: string, max = 80) {
  if (!value) return "";
  if (value.length <= max) return value;
  return `${value.slice(0, max)}…`;
}

type ReverseProxyKeywordDetection = {
  anyHit: boolean;
  hits: string[];
  sources: {
    responseHeaders: { hits: string[]; headerNames: string[] };
    outputPreview: { hits: string[] };
    rawExcerpt: { hits: string[] };
  };
};

export function detectReverseProxyKeywords(
  result: ClaudeModelValidationResult | null
): ReverseProxyKeywordDetection {
  const empty: ReverseProxyKeywordDetection = {
    anyHit: false,
    hits: [],
    sources: {
      responseHeaders: { hits: [], headerNames: [] },
      outputPreview: { hits: [] },
      rawExcerpt: { hits: [] },
    },
  };
  if (!result) return empty;

  const outputPreview =
    typeof result.output_text_preview === "string" ? result.output_text_preview : "";
  const rawExcerpt = typeof result.raw_excerpt === "string" ? result.raw_excerpt : "";

  const outputPreviewHits = listWordBoundaryHits(outputPreview, REVERSE_PROXY_KEYWORDS);
  const rawExcerptHits = listWordBoundaryHits(rawExcerpt, REVERSE_PROXY_KEYWORDS);

  const headerHitKeywords = new Set<string>();
  const headerHitNames = new Set<string>();

  if (isPlainObject(result.response_headers)) {
    for (const [headerName, headerValue] of Object.entries(result.response_headers)) {
      const values = Array.isArray(headerValue)
        ? headerValue.filter((v): v is string => typeof v === "string")
        : typeof headerValue === "string"
          ? [headerValue]
          : [];

      for (const keyword of REVERSE_PROXY_KEYWORDS) {
        const re = new RegExp(`\\b${escapeRegExp(keyword)}\\b`, "i");
        if (re.test(headerName) || values.some((v) => re.test(v))) {
          headerHitKeywords.add(keyword);
          headerHitNames.add(headerName);
        }
      }
    }
  }

  const responseHeaderHits = [...headerHitKeywords].sort((a, b) => a.localeCompare(b));
  const responseHeaderNames = [...headerHitNames].sort((a, b) => a.localeCompare(b));
  const allHits = [
    ...new Set([...responseHeaderHits, ...outputPreviewHits, ...rawExcerptHits]),
  ].sort((a, b) => a.localeCompare(b));

  return {
    anyHit: allHits.length > 0,
    hits: allHits,
    sources: {
      responseHeaders: { hits: responseHeaderHits, headerNames: responseHeaderNames },
      outputPreview: { hits: outputPreviewHits },
      rawExcerpt: { hits: rawExcerptHits },
    },
  };
}

export function listClaudeValidationTemplates(): ClaudeValidationTemplate[] {
  return [...CLAUDE_VALIDATION_TEMPLATES];
}

export function getClaudeValidationTemplate(
  key: string | null | undefined
): ClaudeValidationTemplate {
  const normalized = typeof key === "string" ? key.trim() : "";
  if (normalized) {
    const found = CLAUDE_VALIDATION_TEMPLATES.find((t) => t.key === normalized);
    if (found) return found;
  }

  return (
    CLAUDE_VALIDATION_TEMPLATES.find((t) => t.key === DEFAULT_CLAUDE_VALIDATION_TEMPLATE_KEY) ??
    CLAUDE_VALIDATION_TEMPLATES[0]
  );
}

export function buildClaudeValidationRequestJson(
  templateKey: ClaudeValidationTemplateKey,
  model: string,
  apiKeyPlaintext: string | null
) {
  const template = getClaudeValidationTemplate(templateKey);
  const normalizedModel = model.trim();

  const sessionId = newUuidV4();
  const metadataUserId = buildClaudeCliMetadataUserId(sessionId);

  const baseBody = template.request.body as unknown;
  const nextBody: Record<string, unknown> = isPlainObject(baseBody) ? { ...baseBody } : {};

  nextBody.model = normalizedModel;

  const existingMetadata = isPlainObject(nextBody.metadata)
    ? { ...(nextBody.metadata as any) }
    : {};
  existingMetadata.user_id = metadataUserId;
  nextBody.metadata = existingMetadata;

  const expect = template.request.expect as ClaudeValidationExpect | undefined;
  const headerOverrides = template.request.headers as unknown;

  const wrapper: Record<string, unknown> = {
    template_key: template.key,
    path: template.request.path,
    headers: {
      ...buildClaudeCliValidateHeaders(apiKeyPlaintext),
      ...(isPlainObject(headerOverrides) ? (headerOverrides as Record<string, unknown>) : {}),
    },
    body: nextBody,
  };

  if (typeof template.request.query === "string" && template.request.query.trim()) {
    wrapper.query = template.request.query.trim();
  }

  if (
    expect &&
    (typeof expect.max_output_chars === "number" || typeof expect.exact_output_chars === "number")
  ) {
    wrapper.expect = expect;
  }

  return JSON.stringify(wrapper, null, 2);
}

export function extractTemplateKeyFromRequestJson(requestJson: string): string | null {
  const raw = requestJson.trim();
  if (!raw) return null;
  try {
    const obj = JSON.parse(raw);
    if (!isPlainObject(obj)) return null;
    const key = obj.template_key;
    if (typeof key !== "string") return null;
    const trimmed = key.trim();
    return trimmed ? trimmed : null;
  } catch {
    return null;
  }
}

type OutputExpectation = { kind: "max"; maxChars: number } | { kind: "exact"; exactChars: number };

export type ClaudeValidationOutputExpectation = OutputExpectation;

export function getClaudeValidationOutputExpectation(
  template: ClaudeValidationTemplate
): ClaudeValidationOutputExpectation | null {
  const expect = template.request.expect as ClaudeValidationExpect | undefined;
  const maxChars = typeof expect?.max_output_chars === "number" ? expect.max_output_chars : null;
  if (maxChars != null && Number.isFinite(maxChars) && maxChars > 0) {
    return { kind: "max", maxChars };
  }
  const exactChars =
    typeof expect?.exact_output_chars === "number" ? expect.exact_output_chars : null;
  if (exactChars != null && Number.isFinite(exactChars) && exactChars > 0) {
    return { kind: "exact", exactChars };
  }
  return null;
}

export type ClaudeValidationEvaluation = {
  template: ClaudeValidationTemplate;
  templateKey: ClaudeValidationTemplateKey;
  overallPass: boolean | null;
  checks: {
    cacheDetail?: { ok: boolean; label: string; title: string };
    outputChars?: { ok: boolean; label: string; title: string };
    sseStopReasonMaxTokens?: { ok: boolean; label: string; title: string };
    modelConsistency?: { ok: boolean; label: string; title: string };
    thinkingOutput?: { ok: boolean; label: string; title: string };
    signature?: { ok: boolean; label: string; title: string };
    responseId?: { ok: boolean; label: string; title: string };
    serviceTier?: { ok: boolean; label: string; title: string };
    outputConfig?: { ok: boolean; label: string; title: string };
    toolSupport?: { ok: boolean; label: string; title: string };
    multiTurn?: { ok: boolean; label: string; title: string };
    reverseProxy?: { ok: boolean; label: string; title: string };
  };
  derived: {
    requestedModel: string | null;
    respondedModel: string | null;
    modelConsistency: boolean | null;
    modelName: string;
    outputChars: number;
    thinkingChars: number;
    signatureChars: number;
    hasResponseId: boolean | null;
    hasServiceTier: boolean | null;
    hasError: boolean;
    errorText: string;
  };
};

export function evaluateClaudeValidation(
  templateKeyLike: string | null | undefined,
  result: ClaudeModelValidationResult | null
): ClaudeValidationEvaluation {
  const template = getClaudeValidationTemplate(templateKeyLike);

  const requestedModel =
    typeof result?.requested_model === "string" && result.requested_model.trim()
      ? result.requested_model.trim()
      : null;
  const respondedModel =
    typeof result?.responded_model === "string" && result.responded_model.trim()
      ? result.responded_model.trim()
      : null;

  const modelConsistency =
    requestedModel && respondedModel ? requestedModel === respondedModel : null;
  const modelName = respondedModel ?? requestedModel ?? "—";

  const outputChars = result?.output_text_chars ?? 0;
  const checksRaw = result?.checks as unknown;
  const signalsRaw = result?.signals as unknown;
  const outputPreview =
    typeof result?.output_text_preview === "string" ? result.output_text_preview : "";
  const thinkingPreview = (() => {
    const raw = get<string>(signalsRaw, "thinking_preview");
    if (typeof raw !== "string") return "";
    return raw.trim() ? raw : "";
  })();

  const thinkingBlockSeen = get<boolean>(signalsRaw, "thinking_block_seen");
  const thinkingChars = (() => {
    const fromChecks = get<number>(checksRaw, "thinking_chars");
    if (typeof fromChecks === "number" && Number.isFinite(fromChecks)) return fromChecks;
    const fromSignals = get<number>(signalsRaw, "thinking_chars");
    if (typeof fromSignals === "number" && Number.isFinite(fromSignals)) return fromSignals;
    return 0;
  })();

  const signatureChars = (() => {
    const fromChecks = get<number>(checksRaw, "signature_chars");
    if (typeof fromChecks === "number" && Number.isFinite(fromChecks)) return fromChecks;
    const fromSignals = get<number>(signalsRaw, "signature_chars");
    if (typeof fromSignals === "number" && Number.isFinite(fromSignals)) return fromSignals;
    return 0;
  })();

  const responseIdRaw = get<string>(signalsRaw, "response_id");
  const responseId =
    typeof responseIdRaw === "string" && responseIdRaw.trim() ? responseIdRaw.trim() : null;
  const serviceTierRaw = get<string>(signalsRaw, "service_tier");
  const serviceTier =
    typeof serviceTierRaw === "string" && serviceTierRaw.trim() ? serviceTierRaw.trim() : null;

  const hasResponseId = (() => {
    const fromChecks = get<boolean>(checksRaw, "has_response_id");
    if (typeof fromChecks === "boolean") return fromChecks;
    if (responseId) return true;
    if (result) return false;
    return null;
  })();

  const hasServiceTier = (() => {
    const fromChecks = get<boolean>(checksRaw, "has_service_tier");
    if (typeof fromChecks === "boolean") return fromChecks;
    if (serviceTier) return true;
    if (result) return false;
    return null;
  })();

  const errorText = result?.error ? String(result.error) : "";
  const hasError = Boolean(errorText.trim());

  const checksOut: ClaudeValidationEvaluation["checks"] = {};

  const requireCacheDetail = template.evaluation.requireCacheDetail;
  const requireModelConsistency = template.evaluation.requireModelConsistency;
  const requireThinkingOutput = template.evaluation.requireThinkingOutput;
  const requireSignature = template.evaluation.requireSignature;
  const signatureMinChars = (() => {
    const v = template.evaluation.signatureMinChars;
    return typeof v === "number" && Number.isFinite(v) && v > 0 ? Math.floor(v) : 0;
  })();
  const requireResponseId = template.evaluation.requireResponseId;
  const requireServiceTier = template.evaluation.requireServiceTier;
  const requireOutputConfig = template.evaluation.requireOutputConfig;
  const requireToolSupport = template.evaluation.requireToolSupport;
  const requireMultiTurn = template.evaluation.requireMultiTurn;
  const requireSseStopReasonMaxTokens = template.evaluation.requireSseStopReasonMaxTokens;
  const multiTurnSecretRaw = template.evaluation.multiTurnSecret;
  const multiTurnSecret =
    typeof multiTurnSecretRaw === "string" && multiTurnSecretRaw.trim()
      ? multiTurnSecretRaw.trim()
      : "";

  const capabilityHaystack = `${outputPreview}\n${thinkingPreview}`.trim();

  const reverseProxy = detectReverseProxyKeywords(result);
  if (result) {
    const headerSummary =
      reverseProxy.sources.responseHeaders.hits.length > 0
        ? `headers(${formatHitSummary(reverseProxy.sources.responseHeaders.hits)})`
        : "";
    const outputSummary =
      reverseProxy.sources.outputPreview.hits.length > 0
        ? `output(${formatHitSummary(reverseProxy.sources.outputPreview.hits)})`
        : "";
    const sseSummary =
      reverseProxy.sources.rawExcerpt.hits.length > 0
        ? `sse(${formatHitSummary(reverseProxy.sources.rawExcerpt.hits)})`
        : "";
    const where = [headerSummary, outputSummary, sseSummary].filter(Boolean).join("; ");

    checksOut.reverseProxy = {
      ok: !reverseProxy.anyHit,
      label: "逆向/反代",
      title: reverseProxy.anyHit
        ? `命中：${formatHitSummary(reverseProxy.hits) || "—"}${where ? `；${where}` : ""}`
        : `未发现：${REVERSE_PROXY_KEYWORDS.join(", ")}`,
    };
  }

  if (result) {
    const responseParseMode = get<string>(signalsRaw, "response_parse_mode");
    const parsedAsSse = responseParseMode === "sse" || responseParseMode === "sse_fallback";
    const sseMessageDeltaSeen = get<boolean>(checksRaw, "sse_message_delta_seen") === true;
    const sseStopReasonRaw = get<string>(checksRaw, "sse_message_delta_stop_reason");
    const sseStopReason =
      typeof sseStopReasonRaw === "string" && sseStopReasonRaw.trim()
        ? sseStopReasonRaw.trim()
        : null;
    const sseStopReasonIsMaxTokens =
      get<boolean>(checksRaw, "sse_message_delta_stop_reason_is_max_tokens") === true;

    const ok = parsedAsSse && sseMessageDeltaSeen && sseStopReasonIsMaxTokens;
    const title = (() => {
      if (!parsedAsSse) return `非 SSE 解析（parse_mode=${responseParseMode ?? "—"}）`;
      if (!sseMessageDeltaSeen) return "缺少 event=message_delta";
      if (!sseStopReason) return "message_delta 缺少 stop_reason";
      return `stop_reason=${sseStopReason}`;
    })();

    checksOut.sseStopReasonMaxTokens = {
      ok,
      label: "SSE stop_reason=max_tokens",
      title,
    };
  }

  if (requireCacheDetail) {
    const usage = result?.usage as unknown;
    const cache5m = get<number>(usage, "cache_creation_5m_input_tokens");
    const cache1h = get<number>(usage, "cache_creation_1h_input_tokens");
    const ok = typeof cache5m === "number" && typeof cache1h === "number";
    checksOut.cacheDetail = {
      ok,
      label: "Cache 细分",
      title: `cache_creation_5m_input_tokens / 1h: ${typeof cache5m === "number" ? cache5m : "—"} / ${
        typeof cache1h === "number" ? cache1h : "—"
      }`,
    };
  }

  const outputConfigOk = (() => {
    const usage = result?.usage as unknown;
    const hasCacheDetail = get<boolean>(signalsRaw, "has_cache_creation_detail") === true;
    const cache5m = get<number>(usage, "cache_creation_5m_input_tokens");
    const cache1h = get<number>(usage, "cache_creation_1h_input_tokens");
    const cacheCreation = get<number>(usage, "cache_creation_input_tokens");
    const hasAnyCache =
      hasCacheDetail ||
      (typeof cache5m === "number" && Number.isFinite(cache5m)) ||
      (typeof cache1h === "number" && Number.isFinite(cache1h)) ||
      (typeof cacheCreation === "number" && Number.isFinite(cacheCreation));
    return Boolean(serviceTier) || hasAnyCache;
  })();

  if (requireOutputConfig) {
    const usage = result?.usage as unknown;
    const hasCacheDetail = get<boolean>(signalsRaw, "has_cache_creation_detail") === true;
    const cache5m = get<number>(usage, "cache_creation_5m_input_tokens");
    const cache1h = get<number>(usage, "cache_creation_1h_input_tokens");
    const cacheCreation = get<number>(usage, "cache_creation_input_tokens");
    const fields: string[] = [];
    if (serviceTier) fields.push("service_tier");
    if (hasCacheDetail) fields.push("cache_creation_detail");
    if (typeof cache5m === "number" && Number.isFinite(cache5m)) fields.push("cache_creation_5m");
    if (typeof cache1h === "number" && Number.isFinite(cache1h)) fields.push("cache_creation_1h");
    if (typeof cacheCreation === "number" && Number.isFinite(cacheCreation))
      fields.push("cache_creation_input_tokens");
    checksOut.outputConfig = {
      ok: outputConfigOk,
      label: "Output Config",
      title: outputConfigOk
        ? `存在：${fields.join(", ") || "—"}`
        : "未发现 cache_creation / service_tier",
    };
  } else if (result) {
    checksOut.outputConfig = {
      ok: outputConfigOk,
      label: "Output Config",
      title: outputConfigOk
        ? "存在 cache_creation / service_tier"
        : "未发现 cache_creation / service_tier",
    };
  }

  const toolSupportHitsEn = listKeywordHits(capabilityHaystack, TOOL_SUPPORT_KEYWORDS_EN);
  const toolSupportHitsZh = listKeywordHits(capabilityHaystack, TOOL_SUPPORT_KEYWORDS_ZH);
  const toolSupportOk = toolSupportHitsEn.length >= 2;
  if (requireToolSupport) {
    checksOut.toolSupport = {
      ok: toolSupportOk,
      label: "工具支持",
      title: toolSupportOk
        ? `EN 命中 ${toolSupportHitsEn.length}/2：${formatHitSummary(toolSupportHitsEn)}`
        : `EN 命中 ${toolSupportHitsEn.length}/2：${formatHitSummary(toolSupportHitsEn) || "—"}${
            toolSupportHitsZh.length > 0 ? `；ZH：${formatHitSummary(toolSupportHitsZh)}` : ""
          }`,
    };
  } else if (result) {
    checksOut.toolSupport = {
      ok: toolSupportOk,
      label: "工具支持",
      title: toolSupportOk
        ? `EN 命中 ${toolSupportHitsEn.length}/2：${formatHitSummary(toolSupportHitsEn)}`
        : `EN 命中 ${toolSupportHitsEn.length}/2：${formatHitSummary(toolSupportHitsEn) || "—"}${
            toolSupportHitsZh.length > 0 ? `；ZH：${formatHitSummary(toolSupportHitsZh)}` : ""
          }`,
    };
  }

  const multiTurnSecretPattern = (() => {
    if (!multiTurnSecret) return null;
    try {
      return new RegExp(`\\b${escapeRegExp(multiTurnSecret)}\\b`, "i");
    } catch {
      return null;
    }
  })();

  const outputFirstLine = firstNonEmptyLine(outputPreview);
  const multiTurnSecretOnFirstLine = Boolean(
    multiTurnSecretPattern && outputFirstLine && multiTurnSecretPattern.test(outputFirstLine)
  );
  const multiTurnSecretInOutput = Boolean(
    multiTurnSecretPattern && outputPreview && multiTurnSecretPattern.test(outputPreview)
  );
  const multiTurnSecretInThinking = Boolean(
    multiTurnSecretPattern && thinkingPreview && multiTurnSecretPattern.test(thinkingPreview)
  );

  const multiTurnOk = multiTurnSecretOnFirstLine;

  const multiTurnTitle = (() => {
    if (!multiTurnSecretPattern) return "暗号未配置/无效";
    if (!outputPreview.trim()) return "输出为空（无法判断第一行暗号）";
    if (multiTurnSecretOnFirstLine) return `第一行命中暗号：${multiTurnSecret}`;
    const firstLinePreview = outputFirstLine ? truncateText(outputFirstLine, 60) : "—";
    if (multiTurnSecretInOutput || multiTurnSecretInThinking) {
      const where = [
        multiTurnSecretInOutput ? "output" : null,
        multiTurnSecretInThinking ? "thinking" : null,
      ]
        .filter(Boolean)
        .join(", ");
      return `暗号未出现在第一行（first_line=${firstLinePreview}; elsewhere=${where || "—"}）`;
    }
    return `缺少暗号：${multiTurnSecret}`;
  })();

  if (requireMultiTurn) {
    checksOut.multiTurn = {
      ok: multiTurnOk,
      label: "多轮对话",
      title: multiTurnTitle,
    };
  } else if (result) {
    checksOut.multiTurn = {
      ok: multiTurnOk,
      label: "多轮对话",
      title: multiTurnTitle,
    };
  }

  const outputExpectation = getClaudeValidationOutputExpectation(template);
  if (outputExpectation) {
    const checks = result?.checks as unknown;
    let ok = false;
    let title = "";
    if (outputExpectation.kind === "max") {
      const fromServer = get<boolean>(checks, "output_text_chars_le_max");
      ok = typeof fromServer === "boolean" ? fromServer : outputChars <= outputExpectation.maxChars;
      title = `expect.max_output_chars=${outputExpectation.maxChars}`;
      checksOut.outputChars = {
        ok,
        label: `输出≤${outputExpectation.maxChars} (${outputChars})`,
        title,
      };
    } else {
      const fromServer = get<boolean>(checks, "output_text_chars_eq_expected");
      ok =
        typeof fromServer === "boolean" ? fromServer : outputChars === outputExpectation.exactChars;
      title = `expect.exact_output_chars=${outputExpectation.exactChars}`;
      checksOut.outputChars = {
        ok,
        label: `输出=${outputExpectation.exactChars} (${outputChars})`,
        title,
      };
    }
  }

  if (requireModelConsistency && modelConsistency != null) {
    checksOut.modelConsistency = {
      ok: modelConsistency,
      label: "模型一致",
      title: `requested: ${requestedModel ?? "—"}; responded: ${respondedModel ?? "—"}`,
    };
  }

  const thinkingOk = (() => {
    // If we saw an explicit thinking block, treat as "has thinking output".
    if (thinkingBlockSeen === true) return true;
    if (typeof thinkingChars === "number" && thinkingChars > 0) return true;
    return false;
  })();
  if (requireThinkingOutput) {
    checksOut.thinkingOutput = {
      ok: thinkingOk,
      label: "Thinking 输出",
      title: `thinking_chars=${thinkingChars}; block_seen=${thinkingBlockSeen === true ? "true" : "false"}`,
    };
  } else if (result) {
    // Still surface as reference when data exists (keeps UI consistent without forcing PASS/FAIL gate).
    checksOut.thinkingOutput = {
      ok: thinkingOk,
      label: "Thinking 输出",
      title: `thinking_chars=${thinkingChars}; block_seen=${thinkingBlockSeen === true ? "true" : "false"}`,
    };
  }

  const signatureOk =
    signatureMinChars > 0 ? signatureChars >= signatureMinChars : signatureChars > 0;
  if (requireSignature) {
    checksOut.signature = {
      ok: signatureOk,
      label: "Signature",
      title:
        signatureMinChars > 0
          ? `signature_chars=${signatureChars}; min=${signatureMinChars}`
          : `signature_chars=${signatureChars}`,
    };
  } else if (result) {
    checksOut.signature = {
      ok: signatureOk,
      label: "Signature",
      title:
        signatureMinChars > 0
          ? `signature_chars=${signatureChars}; min=${signatureMinChars}`
          : `signature_chars=${signatureChars}`,
    };
  }

  if (requireResponseId) {
    checksOut.responseId = {
      ok: Boolean(responseId),
      label: "response.id",
      title: responseId ? `id=${responseId}` : "缺少 response.id",
    };
  } else if (result) {
    checksOut.responseId = {
      ok: Boolean(responseId),
      label: "response.id",
      title: responseId ? `id=${responseId}` : "缺少 response.id",
    };
  }

  if (requireServiceTier) {
    checksOut.serviceTier = {
      ok: Boolean(serviceTier),
      label: "service_tier",
      title: serviceTier ? `service_tier=${serviceTier}` : "缺少 service_tier",
    };
  } else if (result) {
    checksOut.serviceTier = {
      ok: Boolean(serviceTier),
      label: "service_tier",
      title: serviceTier ? `service_tier=${serviceTier}` : "缺少 service_tier",
    };
  }

  const overallPass = (() => {
    if (!result) return null;
    if (!result.ok) return false;
    if (hasError) return false;

    if (checksOut.reverseProxy && !checksOut.reverseProxy.ok) return false;
    if (
      requireSseStopReasonMaxTokens &&
      checksOut.sseStopReasonMaxTokens &&
      !checksOut.sseStopReasonMaxTokens.ok
    )
      return false;
    if (requireCacheDetail && checksOut.cacheDetail && !checksOut.cacheDetail.ok) return false;
    if (checksOut.outputChars && !checksOut.outputChars.ok) return false;
    if (requireModelConsistency && modelConsistency === false) return false;
    if (requireThinkingOutput && checksOut.thinkingOutput && !checksOut.thinkingOutput.ok)
      return false;
    if (requireSignature && checksOut.signature && !checksOut.signature.ok) return false;
    if (requireResponseId && checksOut.responseId && !checksOut.responseId.ok) return false;
    if (requireServiceTier && checksOut.serviceTier && !checksOut.serviceTier.ok) return false;
    if (requireOutputConfig && checksOut.outputConfig && !checksOut.outputConfig.ok) return false;
    if (requireToolSupport && checksOut.toolSupport && !checksOut.toolSupport.ok) return false;
    if (requireMultiTurn && checksOut.multiTurn && !checksOut.multiTurn.ok) return false;

    return true;
  })();

  return {
    template,
    templateKey: template.key,
    overallPass,
    checks: checksOut,
    derived: {
      requestedModel,
      respondedModel,
      modelConsistency,
      modelName,
      outputChars,
      thinkingChars,
      signatureChars,
      hasResponseId,
      hasServiceTier,
      hasError,
      errorText,
    },
  };
}
