// Usage:
// - Import helpers/components from this module for Home "request logs" list and "realtime traces" cards.
// - Designed to keep status badge / error_code label / session reuse tooltip consistent across the Home page.

import type { CliKey } from "../../services/providers";
import { Tooltip } from "../../ui/Tooltip";

const ERROR_CODE_LABELS: Record<string, string> = {
  GW_ALL_PROVIDERS_UNAVAILABLE: "全部不可用",
  GW_UPSTREAM_ALL_FAILED: "全部失败",
  GW_NO_ENABLED_PROVIDER: "无供应商",
  GW_UPSTREAM_TIMEOUT: "上游超时",
  GW_UPSTREAM_CONNECT_FAILED: "连接失败",
  GW_UPSTREAM_5XX: "上游5XX",
  GW_UPSTREAM_4XX: "上游4XX",
  GW_UPSTREAM_READ_ERROR: "读取错误",
  GW_STREAM_ERROR: "流错误",
  GW_STREAM_ABORTED: "流中断",
  GW_STREAM_IDLE_TIMEOUT: "流空闲超时",
  GW_REQUEST_ABORTED: "请求中断",
  GW_INTERNAL_ERROR: "内部错误",
  GW_BODY_TOO_LARGE: "请求过大",
  GW_INVALID_CLI_KEY: "无效CLI",
  GW_INVALID_BASE_URL: "无效URL",
  GW_PORT_IN_USE: "端口占用",
  GW_RESPONSE_BUILD_ERROR: "响应构建错误",
};

const STATUS_OVERRIDE_ERROR_CODES = new Set(["GW_STREAM_ABORTED", "GW_REQUEST_ABORTED"]);

const SESSION_REUSE_TOOLTIP =
  "同一 session_id 在 5 分钟 TTL 内优先复用上一次成功 provider，减少抖动/提升缓存命中";

export function getErrorCodeLabel(errorCode: string) {
  return ERROR_CODE_LABELS[errorCode] ?? errorCode;
}

export function SessionReuseBadge({ showCustomTooltip }: { showCustomTooltip: boolean }) {
  const className =
    "inline-flex items-center rounded-full bg-indigo-50 border border-indigo-100 px-1.5 py-0.5 text-[10px] font-medium text-indigo-600 cursor-help";
  return showCustomTooltip ? (
    <Tooltip content={SESSION_REUSE_TOOLTIP}>
      <span className={className}>会话复用</span>
    </Tooltip>
  ) : (
    <span className={className} title={SESSION_REUSE_TOOLTIP}>
      会话复用
    </span>
  );
}

export type StatusBadge = {
  text: string;
  tone: string;
  title?: string;
  isErrorOverride: boolean;
};

export function computeStatusBadge(input: {
  status: number | null;
  errorCode: string | null;
  inProgress?: boolean;
}): StatusBadge {
  if (input.inProgress) {
    return {
      text: "进行中",
      tone: "bg-accent/10 text-accent",
      isErrorOverride: false,
    };
  }

  if (input.errorCode && STATUS_OVERRIDE_ERROR_CODES.has(input.errorCode)) {
    return {
      text: getErrorCodeLabel(input.errorCode),
      tone: "bg-amber-50 text-amber-600 border border-amber-200/60",
      title: input.errorCode,
      isErrorOverride: true,
    };
  }

  const text = input.status == null ? "—" : String(input.status);
  const tone =
    input.status != null && input.status >= 200 && input.status < 400
      ? "text-emerald-600 bg-emerald-50/50"
      : input.status != null && input.status >= 400
        ? "text-rose-600 bg-rose-50/50"
        : "text-slate-500 bg-slate-100";
  return { text, tone, isErrorOverride: false };
}

export function computeEffectiveInputTokens(
  cliKey: CliKey | string,
  inputTokens: number | null,
  cacheReadInputTokens: number | null
) {
  if (inputTokens == null) return null;
  const cacheRead = cacheReadInputTokens ?? 0;
  if (cliKey === "codex" || cliKey === "gemini") return Math.max(inputTokens - cacheRead, 0);
  return inputTokens;
}
