import { useMemo } from "react";
import { cn } from "../utils/cn";

export type ProviderChainAttemptLog = {
  attempt_index: number;
  provider_id: number;
  provider_name: string;
  base_url: string;
  outcome: string;
  status: number | null;
  attempt_started_ms?: number | null;
  attempt_duration_ms?: number | null;
};

type ProviderChainAttemptJson = {
  provider_id: number;
  provider_name: string;
  base_url: string;
  outcome: string;
  status: number | null;
  provider_index?: number | null;
  retry_index?: number | null;
  error_category?: string | null;
  error_code?: string | null;
  decision?: string | null;
  reason?: string | null;
  attempt_started_ms?: number | null;
  attempt_duration_ms?: number | null;
};

type ProviderChainAttempt = {
  attempt_index: number;
  provider_id: number;
  provider_name: string;
  base_url: string;
  outcome: string;
  status: number | null;
  attempt_started_ms: number | null;
  attempt_duration_ms: number | null;
  provider_index: number | null;
  retry_index: number | null;
  error_category: string | null;
  error_code: string | null;
  decision: string | null;
  reason: string | null;
};

export function ProviderChainView({
  attemptLogs,
  attemptLogsLoading,
  attemptsJson,
}: {
  attemptLogs: ProviderChainAttemptLog[];
  attemptLogsLoading?: boolean;
  attemptsJson: string | null | undefined;
}) {
  const parsedAttemptsJson = useMemo(() => {
    if (!attemptsJson)
      return { ok: false as const, attempts: null as ProviderChainAttemptJson[] | null };
    try {
      const parsed = JSON.parse(attemptsJson);
      if (!Array.isArray(parsed)) {
        return { ok: false as const, attempts: null };
      }
      return { ok: true as const, attempts: parsed as ProviderChainAttemptJson[] };
    } catch {
      return { ok: false as const, attempts: null };
    }
  }, [attemptsJson]);

  const attempts = useMemo((): ProviderChainAttempt[] | null => {
    const logs = attemptLogs ?? [];
    const jsonAttempts = parsedAttemptsJson.ok ? parsedAttemptsJson.attempts : null;

    if (logs.length === 0 && !jsonAttempts) return null;

    if (logs.length === 0 && jsonAttempts) {
      return jsonAttempts.map((a, index) => ({
        attempt_index: index + 1,
        provider_id: a.provider_id,
        provider_name: a.provider_name,
        base_url: a.base_url,
        outcome: a.outcome,
        status: a.status ?? null,
        attempt_started_ms: a.attempt_started_ms ?? null,
        attempt_duration_ms: a.attempt_duration_ms ?? null,
        provider_index: a.provider_index ?? null,
        retry_index: a.retry_index ?? null,
        error_category: a.error_category ?? null,
        error_code: a.error_code ?? null,
        decision: a.decision ?? null,
        reason: a.reason ?? null,
      }));
    }

    const byAttemptIndex: Record<number, ProviderChainAttemptJson | undefined> = {};
    if (jsonAttempts) {
      for (let i = 0; i < jsonAttempts.length; i += 1) {
        byAttemptIndex[i + 1] = jsonAttempts[i];
      }
    }

    const normalized = logs
      .slice()
      .sort((a, b) => a.attempt_index - b.attempt_index)
      .map((log) => {
        const json = byAttemptIndex[log.attempt_index];
        return {
          attempt_index: log.attempt_index,
          provider_id: log.provider_id ?? json?.provider_id ?? 0,
          provider_name: log.provider_name || json?.provider_name || "未知",
          base_url: log.base_url || json?.base_url || "",
          outcome: log.outcome || json?.outcome || "",
          status: log.status ?? json?.status ?? null,
          attempt_started_ms: log.attempt_started_ms ?? json?.attempt_started_ms ?? null,
          attempt_duration_ms: log.attempt_duration_ms ?? json?.attempt_duration_ms ?? null,
          provider_index: json?.provider_index ?? null,
          retry_index: json?.retry_index ?? null,
          error_category: json?.error_category ?? null,
          error_code: json?.error_code ?? null,
          decision: json?.decision ?? null,
          reason: json?.reason ?? null,
        };
      });

    return normalized;
  }, [attemptLogs, parsedAttemptsJson]);

  const dataSourceLabel = useMemo(() => {
    if (attemptLogsLoading) return "加载中…";
    if (attemptLogs.length > 0) {
      return parsedAttemptsJson.ok
        ? "数据源：尝试日志（实时落库）+ 尝试 JSON（增强字段）"
        : "数据源：尝试日志（实时落库）";
    }
    if (parsedAttemptsJson.ok) return "数据源：尝试 JSON（兜底）";
    return "数据源：尝试 JSON（原始）";
  }, [attemptLogs.length, attemptLogsLoading, parsedAttemptsJson.ok]);

  if (attemptLogsLoading) {
    return <div className="mt-2 text-sm text-slate-600">加载中…</div>;
  }

  if (!attempts) {
    return <div className="mt-2 text-sm text-slate-600">无故障切换尝试。</div>;
  }

  if (attempts.length === 0) {
    return <div className="mt-2 text-sm text-slate-600">无故障切换尝试。</div>;
  }

  const startAttempt = attempts[0] ?? null;
  const finalAttempt = attempts.length > 0 ? attempts[attempts.length - 1] : null;
  const startProviderLabel = startAttempt
    ? startAttempt.provider_name && startAttempt.provider_name !== "未知"
      ? startAttempt.provider_name
      : `未知（id=${startAttempt.provider_id}）`
    : "—";
  const finalProviderLabel = finalAttempt
    ? finalAttempt.provider_name && finalAttempt.provider_name !== "未知"
      ? finalAttempt.provider_name
      : `未知（id=${finalAttempt.provider_id}）`
    : "—";
  const finalSuccess = finalAttempt ? finalAttempt.outcome === "success" : false;

  return (
    <div className="mt-3 space-y-2">
      <div className="flex flex-wrap items-center justify-between gap-2">
        <div className="text-xs text-slate-500">
          {dataSourceLabel}
          {attemptsJson && !parsedAttemptsJson.ok ? (
            <span className="ml-2 rounded-full bg-amber-50 px-2 py-0.5 font-medium text-amber-700">
              尝试 JSON 解析失败
            </span>
          ) : null}
        </div>
      </div>

      <div className="flex flex-wrap items-center gap-2 text-xs text-slate-600">
        <span className="rounded-full bg-slate-100 px-2 py-0.5">
          起始：<span className="font-medium text-slate-800">{startProviderLabel}</span>
        </span>
        <span className="text-slate-400">→</span>
        <span className="rounded-full bg-slate-100 px-2 py-0.5">
          最终：<span className="font-medium text-slate-800">{finalProviderLabel}</span>
        </span>
        {finalAttempt ? (
          <span
            className={cn(
              "rounded-full px-2 py-0.5 font-medium",
              finalSuccess ? "bg-emerald-50 text-emerald-700" : "bg-rose-50 text-rose-700"
            )}
          >
            {finalSuccess ? "成功" : "失败"}
          </span>
        ) : null}
      </div>

      {attempts.map((attempt) => {
        const success = attempt.outcome === "success";
        const isFinal = Boolean(
          finalAttempt && attempt.attempt_index === finalAttempt.attempt_index
        );
        const providerLabel =
          attempt.provider_name && attempt.provider_name !== "未知"
            ? attempt.provider_name
            : `未知（id=${attempt.provider_id}）`;

        return (
          <div
            key={`${attempt.attempt_index}-${attempt.provider_id}-${attempt.base_url}`}
            className={cn(
              "rounded-xl border bg-white px-3 py-2",
              isFinal
                ? success
                  ? "border-emerald-200 bg-emerald-50/40"
                  : "border-rose-200 bg-rose-50/40"
                : "border-slate-200"
            )}
          >
            <div className="flex flex-col gap-2 sm:flex-row sm:items-start sm:justify-between">
              <div className="min-w-0">
                <div className="flex flex-wrap items-center gap-2">
                  <span className="rounded-full bg-slate-100 px-2 py-0.5 text-xs font-medium text-slate-700">
                    #{attempt.attempt_index}
                  </span>
                  {isFinal ? (
                    <span
                      className={cn(
                        "rounded-full px-2 py-0.5 text-xs font-medium",
                        success ? "bg-emerald-50 text-emerald-700" : "bg-rose-50 text-rose-700"
                      )}
                    >
                      最终
                    </span>
                  ) : null}
                  <span className="truncate text-sm font-medium text-slate-900">
                    {providerLabel}
                  </span>
                  {attempt.provider_index != null ? (
                    <span className="rounded-full bg-slate-100 px-2 py-0.5 text-xs text-slate-700">
                      供应商 #{attempt.provider_index}
                    </span>
                  ) : null}
                  {attempt.retry_index != null ? (
                    <span className="rounded-full bg-slate-100 px-2 py-0.5 text-xs text-slate-700">
                      retry #{attempt.retry_index}
                    </span>
                  ) : null}
                  {attempt.attempt_started_ms != null ? (
                    <span className="rounded-full bg-slate-100 px-2 py-0.5 text-xs text-slate-700">
                      +{attempt.attempt_started_ms}ms
                    </span>
                  ) : null}
                  {attempt.attempt_duration_ms != null ? (
                    <span className="rounded-full bg-slate-100 px-2 py-0.5 text-xs text-slate-700">
                      耗时 {attempt.attempt_duration_ms}ms
                    </span>
                  ) : null}
                </div>

                {attempt.base_url ? (
                  <div className="mt-1 truncate font-mono text-xs text-slate-500">
                    {attempt.base_url}
                  </div>
                ) : null}

                <div className="mt-1 flex flex-wrap items-center gap-2 text-xs text-slate-500">
                  <span>
                    provider_id: <span className="font-mono">{attempt.provider_id}</span>
                  </span>
                  {attempt.error_code ? (
                    <span className="rounded-full bg-amber-50 px-2 py-0.5 font-medium text-amber-700">
                      {attempt.error_code}
                    </span>
                  ) : null}
                  {attempt.decision ? (
                    <span className="rounded-full bg-slate-100 px-2 py-0.5 font-medium text-slate-700">
                      {attempt.decision}
                    </span>
                  ) : null}
                  {attempt.reason ? (
                    <span className="max-w-[520px] truncate font-mono text-xs text-slate-500">
                      {attempt.reason}
                    </span>
                  ) : null}
                </div>
              </div>

              <div className="flex flex-wrap items-center gap-2">
                <span className="rounded-full bg-slate-100 px-2 py-0.5 text-xs font-medium text-slate-700">
                  {attempt.status == null ? "—" : attempt.status}
                </span>
                <span
                  className={cn(
                    "rounded-full px-2 py-0.5 text-xs font-medium",
                    success ? "bg-emerald-50 text-emerald-700" : "bg-amber-50 text-amber-700"
                  )}
                >
                  {success ? "成功" : "失败"}
                </span>
                <span className="max-w-[360px] truncate font-mono text-xs text-slate-600">
                  {attempt.outcome}
                </span>
              </div>
            </div>
          </div>
        );
      })}
    </div>
  );
}
