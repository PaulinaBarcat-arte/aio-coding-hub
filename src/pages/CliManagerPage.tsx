import { useEffect, useState, type KeyboardEvent as ReactKeyboardEvent } from "react";
import { openPath } from "@tauri-apps/plugin-opener";
import { toast } from "sonner";
import { Button } from "../ui/Button";
import { Card } from "../ui/Card";
import { Input } from "../ui/Input";
import { SettingsRow } from "../ui/SettingsRow";
import { Switch } from "../ui/Switch";
import {
  cliManagerClaudeEnvSet,
  cliManagerClaudeInfoGet,
  cliManagerCodexInfoGet,
  cliManagerGeminiInfoGet,
  type ClaudeCliInfo,
  type SimpleCliInfo,
} from "../services/cliManager";
import { logToConsole } from "../services/consoleLog";
import { settingsGet, settingsSet, type AppSettings } from "../services/settings";
import { settingsCodexSessionIdCompletionSet } from "../services/settingsCodexSessionIdCompletion";
import { settingsCircuitBreakerNoticeSet } from "../services/settingsCircuitBreakerNotice";
import {
  settingsGatewayRectifierSet,
  type GatewayRectifierSettingsPatch,
} from "../services/settingsGatewayRectifier";
import { cn } from "../utils/cn";
import {
  Shield,
  Bot,
  Terminal,
  Cpu,
  CheckCircle2,
  AlertTriangle,
  ExternalLink,
  RefreshCw,
  FolderOpen,
  FileJson,
} from "lucide-react";

type TabKey = "general" | "claude" | "codex" | "gemini";

const TABS: Array<{ key: TabKey; label: string; icon: React.ElementType }> = [
  { key: "general", label: "通用", icon: Shield },
  { key: "claude", label: "Claude Code", icon: Bot },
  { key: "codex", label: "Codex", icon: Terminal },
  { key: "gemini", label: "Gemini", icon: Cpu },
];

const DEFAULT_RECTIFIER: GatewayRectifierSettingsPatch = {
  intercept_anthropic_warmup_requests: false,
  enable_thinking_signature_rectifier: false,
  enable_response_fixer: false,
  response_fixer_fix_encoding: true,
  response_fixer_fix_sse_format: true,
  response_fixer_fix_truncated_json: true,
};

const MAX_CLAUDE_MCP_TIMEOUT_MS = 24 * 60 * 60 * 1000;

export function CliManagerPage() {
  const [tab, setTab] = useState<TabKey>("general");
  const [appSettings, setAppSettings] = useState<AppSettings | null>(null);

  const [rectifierAvailable, setRectifierAvailable] = useState<
    "checking" | "available" | "unavailable"
  >("checking");
  const [rectifierSaving, setRectifierSaving] = useState(false);
  const [rectifier, setRectifier] = useState<GatewayRectifierSettingsPatch>(DEFAULT_RECTIFIER);
  const [circuitBreakerNoticeEnabled, setCircuitBreakerNoticeEnabled] = useState(false);
  const [circuitBreakerNoticeSaving, setCircuitBreakerNoticeSaving] = useState(false);
  const [codexSessionIdCompletionEnabled, setCodexSessionIdCompletionEnabled] = useState(false);
  const [codexSessionIdCompletionSaving, setCodexSessionIdCompletionSaving] = useState(false);
  const [commonSettingsSaving, setCommonSettingsSaving] = useState(false);
  const [upstreamFirstByteTimeoutSeconds, setUpstreamFirstByteTimeoutSeconds] = useState<number>(0);
  const [upstreamStreamIdleTimeoutSeconds, setUpstreamStreamIdleTimeoutSeconds] =
    useState<number>(0);
  const [upstreamRequestTimeoutNonStreamingSeconds, setUpstreamRequestTimeoutNonStreamingSeconds] =
    useState<number>(0);
  const [providerCooldownSeconds, setProviderCooldownSeconds] = useState<number>(30);
  const [providerBaseUrlPingCacheTtlSeconds, setProviderBaseUrlPingCacheTtlSeconds] =
    useState<number>(60);
  const [circuitBreakerFailureThreshold, setCircuitBreakerFailureThreshold] = useState<number>(5);
  const [circuitBreakerOpenDurationMinutes, setCircuitBreakerOpenDurationMinutes] =
    useState<number>(30);

  const [claudeAvailable, setClaudeAvailable] = useState<"checking" | "available" | "unavailable">(
    "checking"
  );
  const [claudeLoading, setClaudeLoading] = useState(false);
  const [claudeSaving, setClaudeSaving] = useState(false);
  const [claudeInfo, setClaudeInfo] = useState<ClaudeCliInfo | null>(null);
  const [claudeMcpTimeoutMsText, setClaudeMcpTimeoutMsText] = useState<string>("");

  const [codexAvailable, setCodexAvailable] = useState<"checking" | "available" | "unavailable">(
    "checking"
  );
  const [codexLoading, setCodexLoading] = useState(false);
  const [codexInfo, setCodexInfo] = useState<SimpleCliInfo | null>(null);

  const [geminiAvailable, setGeminiAvailable] = useState<"checking" | "available" | "unavailable">(
    "checking"
  );
  const [geminiLoading, setGeminiLoading] = useState(false);
  const [geminiInfo, setGeminiInfo] = useState<SimpleCliInfo | null>(null);

  useEffect(() => {
    let cancelled = false;
    setRectifierAvailable("checking");
    settingsGet()
      .then((settings) => {
        if (cancelled) return;
        if (!settings) {
          setRectifierAvailable("unavailable");
          setAppSettings(null);
          return;
        }
        setRectifierAvailable("available");
        setAppSettings(settings);
        setRectifier({
          intercept_anthropic_warmup_requests: settings.intercept_anthropic_warmup_requests,
          enable_thinking_signature_rectifier: settings.enable_thinking_signature_rectifier,
          enable_response_fixer: settings.enable_response_fixer,
          response_fixer_fix_encoding: settings.response_fixer_fix_encoding,
          response_fixer_fix_sse_format: settings.response_fixer_fix_sse_format,
          response_fixer_fix_truncated_json: settings.response_fixer_fix_truncated_json,
        });
        setCircuitBreakerNoticeEnabled(settings.enable_circuit_breaker_notice ?? false);
        setCodexSessionIdCompletionEnabled(settings.enable_codex_session_id_completion ?? false);
        setUpstreamFirstByteTimeoutSeconds(settings.upstream_first_byte_timeout_seconds);
        setUpstreamStreamIdleTimeoutSeconds(settings.upstream_stream_idle_timeout_seconds);
        setUpstreamRequestTimeoutNonStreamingSeconds(
          settings.upstream_request_timeout_non_streaming_seconds
        );
        setProviderCooldownSeconds(settings.provider_cooldown_seconds);
        setProviderBaseUrlPingCacheTtlSeconds(settings.provider_base_url_ping_cache_ttl_seconds);
        setCircuitBreakerFailureThreshold(settings.circuit_breaker_failure_threshold);
        setCircuitBreakerOpenDurationMinutes(settings.circuit_breaker_open_duration_minutes);
      })
      .catch((err) => {
        if (cancelled) return;
        logToConsole("error", "读取网关整流配置失败", { error: String(err) });
        setRectifierAvailable("available");
        setAppSettings(null);
        toast("读取网关整流配置失败：请查看控制台日志");
      });

    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    if (tab !== "claude") return;
    if (claudeAvailable !== "checking") return;
    void refreshClaudeInfo();
  }, [tab, claudeAvailable]);

  useEffect(() => {
    if (tab !== "codex") return;
    if (codexAvailable !== "checking") return;
    void refreshCodexInfo();
  }, [tab, codexAvailable]);

  useEffect(() => {
    if (tab !== "gemini") return;
    if (geminiAvailable !== "checking") return;
    void refreshGeminiInfo();
  }, [tab, geminiAvailable]);

  async function persistRectifier(patch: Partial<GatewayRectifierSettingsPatch>) {
    if (rectifierSaving) return;
    if (rectifierAvailable !== "available") return;

    const prev = rectifier;
    const next = { ...prev, ...patch };
    setRectifier(next);
    setRectifierSaving(true);
    try {
      const updated = await settingsGatewayRectifierSet(next);
      if (!updated) {
        toast("仅在 Tauri Desktop 环境可用");
        setRectifier(prev);
        return;
      }

      setAppSettings(updated);
      setRectifier({
        intercept_anthropic_warmup_requests: updated.intercept_anthropic_warmup_requests,
        enable_thinking_signature_rectifier: updated.enable_thinking_signature_rectifier,
        enable_response_fixer: updated.enable_response_fixer,
        response_fixer_fix_encoding: updated.response_fixer_fix_encoding,
        response_fixer_fix_sse_format: updated.response_fixer_fix_sse_format,
        response_fixer_fix_truncated_json: updated.response_fixer_fix_truncated_json,
      });
    } catch (err) {
      logToConsole("error", "更新网关整流配置失败", { error: String(err) });
      toast("更新网关整流配置失败：请稍后重试");
      setRectifier(prev);
    } finally {
      setRectifierSaving(false);
    }
  }

  async function persistCircuitBreakerNotice(enable: boolean) {
    if (circuitBreakerNoticeSaving) return;
    if (rectifierAvailable !== "available") return;

    const prev = circuitBreakerNoticeEnabled;
    setCircuitBreakerNoticeEnabled(enable);
    setCircuitBreakerNoticeSaving(true);
    try {
      const updated = await settingsCircuitBreakerNoticeSet(enable);
      if (!updated) {
        toast("仅在 Tauri Desktop 环境可用");
        setCircuitBreakerNoticeEnabled(prev);
        return;
      }

      setAppSettings(updated);
      setCircuitBreakerNoticeEnabled(updated.enable_circuit_breaker_notice ?? enable);
      toast(enable ? "已开启熔断通知" : "已关闭熔断通知");
    } catch (err) {
      logToConsole("error", "更新熔断通知配置失败", { error: String(err) });
      toast("更新熔断通知配置失败：请稍后重试");
      setCircuitBreakerNoticeEnabled(prev);
    } finally {
      setCircuitBreakerNoticeSaving(false);
    }
  }

  async function persistCodexSessionIdCompletion(enable: boolean) {
    if (codexSessionIdCompletionSaving) return;
    if (rectifierAvailable !== "available") return;

    const prev = codexSessionIdCompletionEnabled;
    setCodexSessionIdCompletionEnabled(enable);
    setCodexSessionIdCompletionSaving(true);
    try {
      const updated = await settingsCodexSessionIdCompletionSet(enable);
      if (!updated) {
        toast("仅在 Tauri Desktop 环境可用");
        setCodexSessionIdCompletionEnabled(prev);
        return;
      }

      setAppSettings(updated);
      setCodexSessionIdCompletionEnabled(updated.enable_codex_session_id_completion ?? enable);
      toast(enable ? "已开启 Codex Session ID 补全" : "已关闭 Codex Session ID 补全");
    } catch (err) {
      logToConsole("error", "更新 Codex Session ID 补全配置失败", { error: String(err) });
      toast("更新 Codex Session ID 补全配置失败：请稍后重试");
      setCodexSessionIdCompletionEnabled(prev);
    } finally {
      setCodexSessionIdCompletionSaving(false);
    }
  }

  async function persistCommonSettings(patch: Partial<AppSettings>) {
    if (commonSettingsSaving) return;
    if (rectifierAvailable !== "available") return;
    if (!appSettings) return;

    const prev = appSettings;
    const next: AppSettings = { ...prev, ...patch };
    setAppSettings(next);
    setCommonSettingsSaving(true);
    try {
      const updated = await settingsSet({
        preferred_port: next.preferred_port,
        auto_start: next.auto_start,
        tray_enabled: next.tray_enabled,
        log_retention_days: next.log_retention_days,
        provider_cooldown_seconds: next.provider_cooldown_seconds,
        provider_base_url_ping_cache_ttl_seconds: next.provider_base_url_ping_cache_ttl_seconds,
        upstream_first_byte_timeout_seconds: next.upstream_first_byte_timeout_seconds,
        upstream_stream_idle_timeout_seconds: next.upstream_stream_idle_timeout_seconds,
        upstream_request_timeout_non_streaming_seconds:
          next.upstream_request_timeout_non_streaming_seconds,
        failover_max_attempts_per_provider: next.failover_max_attempts_per_provider,
        failover_max_providers_to_try: next.failover_max_providers_to_try,
        circuit_breaker_failure_threshold: next.circuit_breaker_failure_threshold,
        circuit_breaker_open_duration_minutes: next.circuit_breaker_open_duration_minutes,
      });

      if (!updated) {
        toast("仅在 Tauri Desktop 环境可用");
        setAppSettings(prev);
        return;
      }

      setAppSettings(updated);
      setUpstreamFirstByteTimeoutSeconds(updated.upstream_first_byte_timeout_seconds);
      setUpstreamStreamIdleTimeoutSeconds(updated.upstream_stream_idle_timeout_seconds);
      setUpstreamRequestTimeoutNonStreamingSeconds(
        updated.upstream_request_timeout_non_streaming_seconds
      );
      setProviderCooldownSeconds(updated.provider_cooldown_seconds);
      setProviderBaseUrlPingCacheTtlSeconds(updated.provider_base_url_ping_cache_ttl_seconds);
      setCircuitBreakerFailureThreshold(updated.circuit_breaker_failure_threshold);
      setCircuitBreakerOpenDurationMinutes(updated.circuit_breaker_open_duration_minutes);
      toast("已保存");
    } catch (err) {
      logToConsole("error", "更新通用网关参数失败", { error: String(err) });
      toast("更新通用网关参数失败：请稍后重试");
      setAppSettings(prev);
      setUpstreamFirstByteTimeoutSeconds(prev.upstream_first_byte_timeout_seconds);
      setUpstreamStreamIdleTimeoutSeconds(prev.upstream_stream_idle_timeout_seconds);
      setUpstreamRequestTimeoutNonStreamingSeconds(
        prev.upstream_request_timeout_non_streaming_seconds
      );
      setProviderCooldownSeconds(prev.provider_cooldown_seconds);
      setProviderBaseUrlPingCacheTtlSeconds(prev.provider_base_url_ping_cache_ttl_seconds);
      setCircuitBreakerFailureThreshold(prev.circuit_breaker_failure_threshold);
      setCircuitBreakerOpenDurationMinutes(prev.circuit_breaker_open_duration_minutes);
    } finally {
      setCommonSettingsSaving(false);
    }
  }

  function applyClaudeInfo(info: ClaudeCliInfo) {
    setClaudeInfo(info);
    setClaudeMcpTimeoutMsText(info.mcp_timeout_ms == null ? "" : String(info.mcp_timeout_ms));
  }

  async function refreshClaudeInfo() {
    if (claudeLoading) return;
    setClaudeLoading(true);
    setClaudeAvailable("checking");
    try {
      const info = await cliManagerClaudeInfoGet();
      if (!info) {
        setClaudeAvailable("unavailable");
        setClaudeInfo(null);
        return;
      }
      setClaudeAvailable("available");
      applyClaudeInfo(info);
    } catch (err) {
      logToConsole("error", "读取 Claude Code 信息失败", { error: String(err) });
      setClaudeAvailable("available");
      toast("读取 Claude Code 信息失败：请查看控制台日志");
    } finally {
      setClaudeLoading(false);
    }
  }

  async function refreshCodexInfo() {
    if (codexLoading) return;
    setCodexLoading(true);
    setCodexAvailable("checking");
    try {
      const info = await cliManagerCodexInfoGet();
      if (!info) {
        setCodexAvailable("unavailable");
        setCodexInfo(null);
        return;
      }
      setCodexAvailable("available");
      setCodexInfo(info);
    } catch (err) {
      logToConsole("error", "读取 Codex 信息失败", { error: String(err) });
      setCodexAvailable("available");
      toast("读取 Codex 信息失败：请查看控制台日志");
    } finally {
      setCodexLoading(false);
    }
  }

  async function refreshGeminiInfo() {
    if (geminiLoading) return;
    setGeminiLoading(true);
    setGeminiAvailable("checking");
    try {
      const info = await cliManagerGeminiInfoGet();
      if (!info) {
        setGeminiAvailable("unavailable");
        setGeminiInfo(null);
        return;
      }
      setGeminiAvailable("available");
      setGeminiInfo(info);
    } catch (err) {
      logToConsole("error", "读取 Gemini 信息失败", { error: String(err) });
      setGeminiAvailable("available");
      toast("读取 Gemini 信息失败：请查看控制台日志");
    } finally {
      setGeminiLoading(false);
    }
  }

  async function persistClaudeEnv(input: {
    mcp_timeout_ms: number | null;
    disable_error_reporting: boolean;
  }) {
    if (claudeSaving) return;
    if (claudeAvailable !== "available") return;

    const prev = claudeInfo;
    setClaudeSaving(true);
    try {
      const updated = await cliManagerClaudeEnvSet({
        mcp_timeout_ms: input.mcp_timeout_ms,
        disable_error_reporting: input.disable_error_reporting,
      });
      if (!updated) {
        toast("仅在 Tauri Desktop 环境可用");
        if (prev) applyClaudeInfo(prev);
        return;
      }
      if (prev) {
        applyClaudeInfo({
          ...prev,
          config_dir: updated.config_dir,
          settings_path: updated.settings_path,
          mcp_timeout_ms: updated.mcp_timeout_ms,
          disable_error_reporting: updated.disable_error_reporting,
        });
      } else {
        applyClaudeInfo({
          found: false,
          executable_path: null,
          version: null,
          error: null,
          shell: null,
          resolved_via: "unavailable",
          config_dir: updated.config_dir,
          settings_path: updated.settings_path,
          mcp_timeout_ms: updated.mcp_timeout_ms,
          disable_error_reporting: updated.disable_error_reporting,
        });
      }
      toast("已更新 Claude Code 配置");
    } catch (err) {
      logToConsole("error", "更新 Claude Code 配置失败", { error: String(err) });
      toast("更新 Claude Code 配置失败：请稍后重试");
      if (prev) applyClaudeInfo(prev);
    } finally {
      setClaudeSaving(false);
    }
  }

  async function openClaudeConfigDir() {
    if (!claudeInfo) return;
    try {
      await openPath(claudeInfo.config_dir);
    } catch (err) {
      logToConsole("error", "打开 Claude 配置目录失败", { error: String(err) });
      toast("打开目录失败：请查看控制台日志");
    }
  }

  function blurOnEnter(e: ReactKeyboardEvent<HTMLInputElement>) {
    if (e.key === "Enter") e.currentTarget.blur();
  }

  function normalizeClaudeMcpTimeoutMsOrNull(raw: string): number | null {
    const trimmed = raw.trim();
    if (!trimmed) return null;
    const n = Math.floor(Number(trimmed));
    if (!Number.isFinite(n) || n < 0) return NaN;
    if (n === 0) return null;
    if (n > MAX_CLAUDE_MCP_TIMEOUT_MS) return Infinity;
    return n;
  }

  return (
    <div className="mx-auto max-w-5xl space-y-6 pb-10">
      <div className="flex flex-col gap-1">
        <h1 className="text-2xl font-bold tracking-tight text-slate-900">CLI 管理</h1>
        <p className="text-sm text-slate-500">
          统一管理 CLI 工具的配置与状态（支持 Claude / Codex / Gemini）。
        </p>
      </div>

      <nav className="flex space-x-1 rounded-xl bg-slate-100/50 p-1">
        {TABS.map((item) => {
          const active = tab === item.key;
          const Icon = item.icon;
          return (
            <button
              key={item.key}
              onClick={() => setTab(item.key)}
              className={cn(
                "group flex flex-1 items-center justify-center gap-2 rounded-lg px-3 py-2.5 text-sm font-medium transition-all outline-none focus-visible:ring-2 focus-visible:ring-offset-2 focus-visible:ring-blue-500",
                active
                  ? "bg-white text-blue-600 shadow-sm ring-1 ring-slate-200"
                  : "text-slate-500 hover:bg-slate-200/50 hover:text-slate-900"
              )}
            >
              <Icon
                className={cn(
                  "h-4 w-4",
                  active ? "text-blue-500" : "text-slate-400 group-hover:text-slate-500"
                )}
              />
              {item.label}
            </button>
          );
        })}
      </nav>

      <div className="min-h-[400px]">
        {tab === "general" && (
          <div className="space-y-6">
            <div className="grid gap-6 md:grid-cols-2">
              <Card className="md:col-span-2 relative overflow-hidden">
                <div className="absolute top-0 right-0 p-4 opacity-5">
                  <Shield className="h-32 w-32" />
                </div>
                <div className="relative z-10">
                  <div className="mb-4 border-b border-slate-100 pb-4">
                    <h2 className="text-lg font-semibold text-slate-900 flex items-center gap-2">
                      <Shield className="h-5 w-5 text-blue-500" />
                      网关整流器
                    </h2>
                    <p className="mt-1 text-sm text-slate-500">
                      优化与 AI 服务的连接稳定性，自动修复常见响应问题。
                    </p>
                  </div>

                  {rectifierAvailable === "unavailable" ? (
                    <div className="text-sm text-slate-600 bg-slate-50 p-4 rounded-lg">
                      仅在 Tauri Desktop 环境可用
                    </div>
                  ) : (
                    <div className="space-y-4">
                      <SettingsRow label="拦截 Anthropic Warmup 请求">
                        <Switch
                          checked={rectifier.intercept_anthropic_warmup_requests}
                          onCheckedChange={(checked) =>
                            void persistRectifier({ intercept_anthropic_warmup_requests: checked })
                          }
                          disabled={rectifierSaving || rectifierAvailable !== "available"}
                        />
                      </SettingsRow>
                      <SettingsRow label="Thinking 签名整流器">
                        <Switch
                          checked={rectifier.enable_thinking_signature_rectifier}
                          onCheckedChange={(checked) =>
                            void persistRectifier({ enable_thinking_signature_rectifier: checked })
                          }
                          disabled={rectifierSaving || rectifierAvailable !== "available"}
                        />
                      </SettingsRow>
                      <div className="rounded-lg bg-slate-50 p-4 border border-slate-100">
                        <SettingsRow label="响应整流（FluxFix）">
                          <Switch
                            checked={rectifier.enable_response_fixer}
                            onCheckedChange={(checked) =>
                              void persistRectifier({ enable_response_fixer: checked })
                            }
                            disabled={rectifierSaving || rectifierAvailable !== "available"}
                          />
                        </SettingsRow>
                        {rectifier.enable_response_fixer && (
                          <div className="mt-2 space-y-2 pl-4 border-l-2 border-slate-200 ml-1">
                            <SettingsRow label="修复编码问题">
                              <Switch
                                checked={rectifier.response_fixer_fix_encoding}
                                onCheckedChange={(checked) =>
                                  void persistRectifier({ response_fixer_fix_encoding: checked })
                                }
                                disabled={rectifierSaving || rectifierAvailable !== "available"}
                              />
                            </SettingsRow>
                            <SettingsRow label="修复 SSE 格式">
                              <Switch
                                checked={rectifier.response_fixer_fix_sse_format}
                                onCheckedChange={(checked) =>
                                  void persistRectifier({ response_fixer_fix_sse_format: checked })
                                }
                                disabled={rectifierSaving || rectifierAvailable !== "available"}
                              />
                            </SettingsRow>
                            <SettingsRow label="修复截断的 JSON">
                              <Switch
                                checked={rectifier.response_fixer_fix_truncated_json}
                                onCheckedChange={(checked) =>
                                  void persistRectifier({
                                    response_fixer_fix_truncated_json: checked,
                                  })
                                }
                                disabled={rectifierSaving || rectifierAvailable !== "available"}
                              />
                            </SettingsRow>
                          </div>
                        )}
                      </div>

                      <div className="rounded-lg bg-slate-50 p-4 border border-slate-100">
                        <SettingsRow label="Codex Session ID 补全">
                          <Switch
                            checked={codexSessionIdCompletionEnabled}
                            onCheckedChange={(checked) =>
                              void persistCodexSessionIdCompletion(checked)
                            }
                            disabled={
                              codexSessionIdCompletionSaving || rectifierAvailable !== "available"
                            }
                          />
                        </SettingsRow>
                        <p className="mt-2 text-xs text-slate-500">
                          当 Codex 请求仅提供 session_id / x-session-id（请求头）或
                          prompt_cache_key（请求体）之一时，
                          自动补全另一侧；若两者均缺失，则生成并在短时间内稳定复用的会话标识。
                        </p>
                      </div>
                    </div>
                  )}
                </div>
              </Card>

              <Card className="md:col-span-2">
                <div className="mb-4 flex items-start gap-4">
                  <div className="p-2 bg-amber-50 rounded-lg text-amber-600">
                    <AlertTriangle className="h-6 w-6" />
                  </div>
                  <div className="flex-1">
                    <h3 className="text-base font-semibold text-slate-900">熔断通知</h3>
                    <p className="mt-1 text-sm text-slate-500">
                      当服务熔断触发或恢复时，主动发送系统通知。
                      <br />
                      <span className="text-xs text-amber-600/80">
                        * 需在系统设置中授予通知权限
                      </span>
                    </p>
                  </div>
                  <div className="pt-1">
                    {rectifierAvailable === "unavailable" ? (
                      <span className="text-xs text-slate-400">不可用</span>
                    ) : (
                      <Switch
                        checked={circuitBreakerNoticeEnabled}
                        onCheckedChange={(checked) => void persistCircuitBreakerNotice(checked)}
                        disabled={circuitBreakerNoticeSaving || rectifierAvailable !== "available"}
                      />
                    )}
                  </div>
                </div>
              </Card>

              <Card className="md:col-span-2">
                <div className="mb-4 border-b border-slate-100 pb-4">
                  <div className="font-semibold text-slate-900">超时策略</div>
                  <p className="mt-1 text-sm text-slate-500">
                    控制上游请求的超时行为。0 表示禁用（交由上游/网络自行超时）。
                  </p>
                </div>

                {rectifierAvailable === "unavailable" ? (
                  <div className="text-sm text-slate-600 bg-slate-50 p-4 rounded-lg">
                    仅在 Tauri Desktop 环境可用
                  </div>
                ) : (
                  <div className="space-y-1">
                    <SettingsRow label="首字节超时（0=禁用）">
                      <div className="flex items-center gap-2">
                        <Input
                          type="number"
                          value={upstreamFirstByteTimeoutSeconds}
                          onChange={(e) => {
                            const next = e.currentTarget.valueAsNumber;
                            if (Number.isFinite(next)) setUpstreamFirstByteTimeoutSeconds(next);
                          }}
                          onBlur={(e) => {
                            if (!appSettings) return;
                            const next = e.currentTarget.valueAsNumber;
                            if (!Number.isFinite(next) || next < 0 || next > 3600) {
                              toast("上游首字节超时必须为 0-3600 秒");
                              setUpstreamFirstByteTimeoutSeconds(
                                appSettings.upstream_first_byte_timeout_seconds
                              );
                              return;
                            }
                            void persistCommonSettings({
                              upstream_first_byte_timeout_seconds: next,
                            });
                          }}
                          onKeyDown={blurOnEnter}
                          className="w-24"
                          min={0}
                          max={3600}
                          disabled={commonSettingsSaving || rectifierAvailable !== "available"}
                        />
                        <span className="text-sm text-slate-500">秒</span>
                      </div>
                    </SettingsRow>

                    <SettingsRow label="流式空闲超时（0=禁用）">
                      <div className="flex items-center gap-2">
                        <Input
                          type="number"
                          value={upstreamStreamIdleTimeoutSeconds}
                          onChange={(e) => {
                            const next = e.currentTarget.valueAsNumber;
                            if (Number.isFinite(next)) setUpstreamStreamIdleTimeoutSeconds(next);
                          }}
                          onBlur={(e) => {
                            if (!appSettings) return;
                            const next = e.currentTarget.valueAsNumber;
                            if (!Number.isFinite(next) || next < 0 || next > 3600) {
                              toast("上游流式空闲超时必须为 0-3600 秒");
                              setUpstreamStreamIdleTimeoutSeconds(
                                appSettings.upstream_stream_idle_timeout_seconds
                              );
                              return;
                            }
                            void persistCommonSettings({
                              upstream_stream_idle_timeout_seconds: next,
                            });
                          }}
                          onKeyDown={blurOnEnter}
                          className="w-24"
                          min={0}
                          max={3600}
                          disabled={commonSettingsSaving || rectifierAvailable !== "available"}
                        />
                        <span className="text-sm text-slate-500">秒</span>
                      </div>
                    </SettingsRow>

                    <SettingsRow label="非流式总超时（0=禁用）">
                      <div className="flex items-center gap-2">
                        <Input
                          type="number"
                          value={upstreamRequestTimeoutNonStreamingSeconds}
                          onChange={(e) => {
                            const next = e.currentTarget.valueAsNumber;
                            if (Number.isFinite(next))
                              setUpstreamRequestTimeoutNonStreamingSeconds(next);
                          }}
                          onBlur={(e) => {
                            if (!appSettings) return;
                            const next = e.currentTarget.valueAsNumber;
                            if (!Number.isFinite(next) || next < 0 || next > 86400) {
                              toast("上游非流式总超时必须为 0-86400 秒");
                              setUpstreamRequestTimeoutNonStreamingSeconds(
                                appSettings.upstream_request_timeout_non_streaming_seconds
                              );
                              return;
                            }
                            void persistCommonSettings({
                              upstream_request_timeout_non_streaming_seconds: next,
                            });
                          }}
                          onKeyDown={blurOnEnter}
                          className="w-24"
                          min={0}
                          max={86400}
                          disabled={commonSettingsSaving || rectifierAvailable !== "available"}
                        />
                        <span className="text-sm text-slate-500">秒</span>
                      </div>
                    </SettingsRow>
                  </div>
                )}
              </Card>

              <Card className="md:col-span-2">
                <div className="mb-4 border-b border-slate-100 pb-4">
                  <div className="font-semibold text-slate-900">熔断与重试</div>
                  <p className="mt-1 text-sm text-slate-500">
                    控制 Provider 失败后的冷却、重试与熔断行为。修改后建议重启网关以完全生效。
                  </p>
                </div>

                {rectifierAvailable === "unavailable" ? (
                  <div className="text-sm text-slate-600 bg-slate-50 p-4 rounded-lg">
                    仅在 Tauri Desktop 环境可用
                  </div>
                ) : (
                  <div className="space-y-1">
                    <SettingsRow label="Provider 冷却">
                      <div className="flex items-center gap-2">
                        <Input
                          type="number"
                          value={providerCooldownSeconds}
                          onChange={(e) => {
                            const next = e.currentTarget.valueAsNumber;
                            if (Number.isFinite(next)) setProviderCooldownSeconds(next);
                          }}
                          onBlur={(e) => {
                            if (!appSettings) return;
                            const next = e.currentTarget.valueAsNumber;
                            if (!Number.isFinite(next) || next < 0 || next > 3600) {
                              toast("短熔断冷却必须为 0-3600 秒");
                              setProviderCooldownSeconds(appSettings.provider_cooldown_seconds);
                              return;
                            }
                            void persistCommonSettings({ provider_cooldown_seconds: next });
                          }}
                          onKeyDown={blurOnEnter}
                          className="w-24"
                          min={0}
                          max={3600}
                          disabled={commonSettingsSaving || rectifierAvailable !== "available"}
                        />
                        <span className="text-sm text-slate-500">秒</span>
                      </div>
                    </SettingsRow>

                    <SettingsRow label="Ping 选择缓存 TTL">
                      <div className="flex items-center gap-2">
                        <Input
                          type="number"
                          value={providerBaseUrlPingCacheTtlSeconds}
                          onChange={(e) => {
                            const next = e.currentTarget.valueAsNumber;
                            if (Number.isFinite(next)) setProviderBaseUrlPingCacheTtlSeconds(next);
                          }}
                          onBlur={(e) => {
                            if (!appSettings) return;
                            const next = e.currentTarget.valueAsNumber;
                            if (!Number.isFinite(next) || next < 1 || next > 3600) {
                              toast("Ping 选择缓存 TTL 必须为 1-3600 秒");
                              setProviderBaseUrlPingCacheTtlSeconds(
                                appSettings.provider_base_url_ping_cache_ttl_seconds
                              );
                              return;
                            }
                            void persistCommonSettings({
                              provider_base_url_ping_cache_ttl_seconds: next,
                            });
                          }}
                          onKeyDown={blurOnEnter}
                          className="w-24"
                          min={1}
                          max={3600}
                          disabled={commonSettingsSaving || rectifierAvailable !== "available"}
                        />
                        <span className="text-sm text-slate-500">秒</span>
                      </div>
                    </SettingsRow>

                    <SettingsRow label="熔断阈值">
                      <div className="flex items-center gap-2">
                        <Input
                          type="number"
                          value={circuitBreakerFailureThreshold}
                          onChange={(e) => {
                            const next = e.currentTarget.valueAsNumber;
                            if (Number.isFinite(next)) setCircuitBreakerFailureThreshold(next);
                          }}
                          onBlur={(e) => {
                            if (!appSettings) return;
                            const next = e.currentTarget.valueAsNumber;
                            if (!Number.isFinite(next) || next < 1 || next > 50) {
                              toast("熔断阈值必须为 1-50");
                              setCircuitBreakerFailureThreshold(
                                appSettings.circuit_breaker_failure_threshold
                              );
                              return;
                            }
                            void persistCommonSettings({ circuit_breaker_failure_threshold: next });
                          }}
                          onKeyDown={blurOnEnter}
                          className="w-24"
                          min={1}
                          max={50}
                          disabled={commonSettingsSaving || rectifierAvailable !== "available"}
                        />
                        <span className="text-sm text-slate-500">次</span>
                      </div>
                    </SettingsRow>

                    <SettingsRow label="熔断时长">
                      <div className="flex items-center gap-2">
                        <Input
                          type="number"
                          value={circuitBreakerOpenDurationMinutes}
                          onChange={(e) => {
                            const next = e.currentTarget.valueAsNumber;
                            if (Number.isFinite(next)) setCircuitBreakerOpenDurationMinutes(next);
                          }}
                          onBlur={(e) => {
                            if (!appSettings) return;
                            const next = e.currentTarget.valueAsNumber;
                            if (!Number.isFinite(next) || next < 1 || next > 1440) {
                              toast("熔断时长必须为 1-1440 分钟");
                              setCircuitBreakerOpenDurationMinutes(
                                appSettings.circuit_breaker_open_duration_minutes
                              );
                              return;
                            }
                            void persistCommonSettings({
                              circuit_breaker_open_duration_minutes: next,
                            });
                          }}
                          onKeyDown={blurOnEnter}
                          className="w-24"
                          min={1}
                          max={1440}
                          disabled={commonSettingsSaving || rectifierAvailable !== "available"}
                        />
                        <span className="text-sm text-slate-500">分钟</span>
                      </div>
                    </SettingsRow>
                  </div>
                )}
              </Card>
            </div>
          </div>
        )}

        {tab === "claude" && (
          <div className="space-y-6">
            <Card className="overflow-hidden">
              <div className="flex flex-col md:flex-row items-start md:items-center justify-between gap-4 border-b border-slate-100 pb-6 mb-6">
                <div className="flex items-center gap-4">
                  <div className="h-14 w-14 rounded-xl bg-[#D97757]/10 flex items-center justify-center text-[#D97757]">
                    <Bot className="h-8 w-8" />
                  </div>
                  <div>
                    <h2 className="text-xl font-bold text-slate-900">Claude Code</h2>
                    <div className="flex items-center gap-2 mt-1">
                      {claudeAvailable === "available" && claudeInfo?.found ? (
                        <span className="inline-flex items-center gap-1.5 rounded-full bg-green-50 px-2.5 py-0.5 text-xs font-medium text-green-700 ring-1 ring-inset ring-green-600/20">
                          <CheckCircle2 className="h-3 w-3" />
                          已安装 {claudeInfo.version}
                        </span>
                      ) : claudeAvailable === "checking" || claudeLoading ? (
                        <span className="inline-flex items-center gap-1.5 rounded-full bg-blue-50 px-2.5 py-0.5 text-xs font-medium text-blue-700 ring-1 ring-inset ring-blue-600/20">
                          <RefreshCw className="h-3 w-3 animate-spin" />
                          检测中...
                        </span>
                      ) : (
                        <span className="inline-flex items-center gap-1.5 rounded-full bg-slate-100 px-2.5 py-0.5 text-xs font-medium text-slate-600 ring-1 ring-inset ring-slate-500/10">
                          未检测到
                        </span>
                      )}
                    </div>
                  </div>
                </div>
                <Button
                  onClick={() => void refreshClaudeInfo()}
                  variant="secondary"
                  size="sm"
                  disabled={claudeLoading}
                  className="gap-2"
                >
                  <RefreshCw className={cn("h-3.5 w-3.5", claudeLoading && "animate-spin")} />
                  刷新状态
                </Button>
              </div>

              {claudeAvailable === "unavailable" ? (
                <div className="text-sm text-slate-600 text-center py-8">
                  仅在 Tauri Desktop 环境可用
                </div>
              ) : !claudeInfo ? (
                <div className="text-sm text-slate-500 text-center py-8">暂无信息，请尝试刷新</div>
              ) : (
                <div className="grid gap-6 md:grid-cols-2">
                  <div className="space-y-4">
                    <h3 className="text-sm font-semibold text-slate-900 flex items-center gap-2">
                      <FolderOpen className="h-4 w-4 text-slate-400" />
                      路径信息
                    </h3>
                    <div className="space-y-3">
                      <div>
                        <div className="text-xs text-slate-500 mb-1">可执行文件</div>
                        <div className="font-mono text-xs text-slate-700 bg-slate-50 p-2 rounded border border-slate-100 break-all">
                          {claudeInfo.executable_path ?? "—"}
                        </div>
                      </div>
                      <div>
                        <div className="text-xs text-slate-500 mb-1">SHELL ($SHELL)</div>
                        <div className="font-mono text-xs text-slate-700 bg-slate-50 p-2 rounded border border-slate-100 break-all">
                          {claudeInfo.shell ?? "—"}
                        </div>
                      </div>
                      <div>
                        <div className="text-xs text-slate-500 mb-1">解析方式</div>
                        <div className="font-mono text-xs text-slate-700 bg-slate-50 p-2 rounded border border-slate-100 break-all">
                          {claudeInfo.resolved_via}
                        </div>
                      </div>
                      <div>
                        <div className="text-xs text-slate-500 mb-1">配置目录</div>
                        <div className="flex gap-2">
                          <div className="font-mono text-xs text-slate-700 bg-slate-50 p-2 rounded border border-slate-100 break-all flex-1">
                            {claudeInfo.config_dir}
                          </div>
                          <Button
                            onClick={() => void openClaudeConfigDir()}
                            size="sm"
                            variant="secondary"
                            className="shrink-0 h-auto py-1"
                          >
                            <ExternalLink className="h-3 w-3" />
                          </Button>
                        </div>
                      </div>
                      <div>
                        <div className="text-xs text-slate-500 mb-1">settings.json</div>
                        <div className="font-mono text-xs text-slate-700 bg-slate-50 p-2 rounded border border-slate-100 break-all">
                          {claudeInfo.settings_path}
                        </div>
                      </div>
                    </div>
                  </div>

                  <div className="space-y-4">
                    <h3 className="text-sm font-semibold text-slate-900 flex items-center gap-2">
                      <FileJson className="h-4 w-4 text-slate-400" />
                      环境配置 (env)
                    </h3>
                    <div className="rounded-lg border border-slate-200 bg-white p-4 space-y-4">
                      <div>
                        <label className="text-sm font-medium text-slate-700 mb-1 block">
                          MCP_TIMEOUT (ms)
                        </label>
                        <div className="flex gap-2">
                          <Input
                            type="number"
                            value={claudeMcpTimeoutMsText}
                            onChange={(e) => setClaudeMcpTimeoutMsText(e.currentTarget.value)}
                            onBlur={() => {
                              if (!claudeInfo) return;
                              const normalized =
                                normalizeClaudeMcpTimeoutMsOrNull(claudeMcpTimeoutMsText);
                              if (normalized !== null && !Number.isFinite(normalized)) {
                                toast(`MCP_TIMEOUT 必须为 0-${MAX_CLAUDE_MCP_TIMEOUT_MS} 毫秒`);
                                setClaudeMcpTimeoutMsText(
                                  claudeInfo.mcp_timeout_ms == null
                                    ? ""
                                    : String(claudeInfo.mcp_timeout_ms)
                                );
                                return;
                              }
                              void persistClaudeEnv({
                                mcp_timeout_ms: normalized,
                                disable_error_reporting: claudeInfo.disable_error_reporting,
                              });
                            }}
                            onKeyDown={blurOnEnter}
                            className="font-mono"
                            min={0}
                            max={MAX_CLAUDE_MCP_TIMEOUT_MS}
                            disabled={claudeSaving}
                            placeholder="默认"
                          />
                        </div>
                        <p className="mt-1.5 text-xs text-slate-500">
                          MCP 连接超时时间。留空或 0 表示使用默认值。
                        </p>
                      </div>

                      <div className="flex items-center justify-between py-2">
                        <div>
                          <div className="text-sm font-medium text-slate-700">
                            DISABLE_ERROR_REPORTING
                          </div>
                          <div className="text-xs text-slate-500">禁用错误上报功能</div>
                        </div>
                        <Switch
                          checked={claudeInfo.disable_error_reporting}
                          onCheckedChange={(checked) => {
                            void persistClaudeEnv({
                              mcp_timeout_ms: claudeInfo.mcp_timeout_ms,
                              disable_error_reporting: checked,
                            });
                          }}
                          disabled={claudeSaving}
                        />
                      </div>
                    </div>
                  </div>
                </div>
              )}
              {claudeInfo?.error && (
                <div className="mt-4 rounded-lg bg-rose-50 p-4 text-sm text-rose-600 flex items-start gap-2">
                  <AlertTriangle className="h-5 w-5 shrink-0" />
                  <div>
                    <span className="font-semibold">检测失败：</span>
                    {claudeInfo.error}
                  </div>
                </div>
              )}
            </Card>
          </div>
        )}

        {tab === "codex" && (
          <div className="space-y-6">
            <Card className="overflow-hidden">
              <div className="flex flex-col md:flex-row items-start md:items-center justify-between gap-4 border-b border-slate-100 pb-6 mb-6">
                <div className="flex items-center gap-4">
                  <div className="h-14 w-14 rounded-xl bg-slate-900/5 flex items-center justify-center text-slate-700">
                    <Terminal className="h-8 w-8" />
                  </div>
                  <div>
                    <h2 className="text-xl font-bold text-slate-900">Codex</h2>
                    <div className="flex items-center gap-2 mt-1">
                      {codexAvailable === "available" && codexInfo?.found ? (
                        <span className="inline-flex items-center gap-1.5 rounded-full bg-green-50 px-2.5 py-0.5 text-xs font-medium text-green-700 ring-1 ring-inset ring-green-600/20">
                          <CheckCircle2 className="h-3 w-3" />
                          已安装 {codexInfo.version}
                        </span>
                      ) : codexAvailable === "checking" || codexLoading ? (
                        <span className="inline-flex items-center gap-1.5 rounded-full bg-blue-50 px-2.5 py-0.5 text-xs font-medium text-blue-700 ring-1 ring-inset ring-blue-600/20">
                          <RefreshCw className="h-3 w-3 animate-spin" />
                          检测中...
                        </span>
                      ) : (
                        <span className="inline-flex items-center gap-1.5 rounded-full bg-slate-100 px-2.5 py-0.5 text-xs font-medium text-slate-600 ring-1 ring-inset ring-slate-500/10">
                          未检测到
                        </span>
                      )}
                    </div>
                  </div>
                </div>
                <Button
                  onClick={() => void refreshCodexInfo()}
                  variant="secondary"
                  size="sm"
                  disabled={codexLoading}
                  className="gap-2"
                >
                  <RefreshCw className={cn("h-3.5 w-3.5", codexLoading && "animate-spin")} />
                  刷新状态
                </Button>
              </div>

              {codexAvailable === "unavailable" ? (
                <div className="text-sm text-slate-600 text-center py-8">
                  仅在 Tauri Desktop 环境可用
                </div>
              ) : !codexInfo ? (
                <div className="text-sm text-slate-500 text-center py-8">暂无信息，请尝试刷新</div>
              ) : (
                <div className="grid gap-6 md:grid-cols-2">
                  <div className="space-y-4">
                    <h3 className="text-sm font-semibold text-slate-900 flex items-center gap-2">
                      <FolderOpen className="h-4 w-4 text-slate-400" />
                      路径信息
                    </h3>
                    <div className="space-y-3">
                      <div>
                        <div className="text-xs text-slate-500 mb-1">可执行文件</div>
                        <div className="font-mono text-xs text-slate-700 bg-slate-50 p-2 rounded border border-slate-100 break-all">
                          {codexInfo.executable_path ?? "—"}
                        </div>
                      </div>
                      <div>
                        <div className="text-xs text-slate-500 mb-1">版本</div>
                        <div className="font-mono text-xs text-slate-700 bg-slate-50 p-2 rounded border border-slate-100 break-all">
                          {codexInfo.version ?? "—"}
                        </div>
                      </div>
                    </div>
                  </div>

                  <div className="space-y-4">
                    <h3 className="text-sm font-semibold text-slate-900 flex items-center gap-2">
                      <FileJson className="h-4 w-4 text-slate-400" />
                      解析环境
                    </h3>
                    <div className="space-y-3">
                      <div>
                        <div className="text-xs text-slate-500 mb-1">$SHELL</div>
                        <div className="font-mono text-xs text-slate-700 bg-slate-50 p-2 rounded border border-slate-100 break-all">
                          {codexInfo.shell ?? "—"}
                        </div>
                      </div>
                      <div>
                        <div className="text-xs text-slate-500 mb-1">解析方式</div>
                        <div className="font-mono text-xs text-slate-700 bg-slate-50 p-2 rounded border border-slate-100 break-all">
                          {codexInfo.resolved_via}
                        </div>
                      </div>
                    </div>
                  </div>
                </div>
              )}

              {codexInfo?.error && (
                <div className="mt-4 rounded-lg bg-rose-50 p-4 text-sm text-rose-600 flex items-start gap-2">
                  <AlertTriangle className="h-5 w-5 shrink-0" />
                  <div>
                    <span className="font-semibold">检测失败：</span>
                    {codexInfo.error}
                  </div>
                </div>
              )}
            </Card>
          </div>
        )}

        {tab === "gemini" && (
          <div className="space-y-6">
            <Card className="overflow-hidden">
              <div className="flex flex-col md:flex-row items-start md:items-center justify-between gap-4 border-b border-slate-100 pb-6 mb-6">
                <div className="flex items-center gap-4">
                  <div className="h-14 w-14 rounded-xl bg-slate-900/5 flex items-center justify-center text-slate-700">
                    <Cpu className="h-8 w-8" />
                  </div>
                  <div>
                    <h2 className="text-xl font-bold text-slate-900">Gemini</h2>
                    <div className="flex items-center gap-2 mt-1">
                      {geminiAvailable === "available" && geminiInfo?.found ? (
                        <span className="inline-flex items-center gap-1.5 rounded-full bg-green-50 px-2.5 py-0.5 text-xs font-medium text-green-700 ring-1 ring-inset ring-green-600/20">
                          <CheckCircle2 className="h-3 w-3" />
                          已安装 {geminiInfo.version}
                        </span>
                      ) : geminiAvailable === "checking" || geminiLoading ? (
                        <span className="inline-flex items-center gap-1.5 rounded-full bg-blue-50 px-2.5 py-0.5 text-xs font-medium text-blue-700 ring-1 ring-inset ring-blue-600/20">
                          <RefreshCw className="h-3 w-3 animate-spin" />
                          检测中...
                        </span>
                      ) : (
                        <span className="inline-flex items-center gap-1.5 rounded-full bg-slate-100 px-2.5 py-0.5 text-xs font-medium text-slate-600 ring-1 ring-inset ring-slate-500/10">
                          未检测到
                        </span>
                      )}
                    </div>
                  </div>
                </div>
                <Button
                  onClick={() => void refreshGeminiInfo()}
                  variant="secondary"
                  size="sm"
                  disabled={geminiLoading}
                  className="gap-2"
                >
                  <RefreshCw className={cn("h-3.5 w-3.5", geminiLoading && "animate-spin")} />
                  刷新状态
                </Button>
              </div>

              {geminiAvailable === "unavailable" ? (
                <div className="text-sm text-slate-600 text-center py-8">
                  仅在 Tauri Desktop 环境可用
                </div>
              ) : !geminiInfo ? (
                <div className="text-sm text-slate-500 text-center py-8">暂无信息，请尝试刷新</div>
              ) : (
                <div className="grid gap-6 md:grid-cols-2">
                  <div className="space-y-4">
                    <h3 className="text-sm font-semibold text-slate-900 flex items-center gap-2">
                      <FolderOpen className="h-4 w-4 text-slate-400" />
                      路径信息
                    </h3>
                    <div className="space-y-3">
                      <div>
                        <div className="text-xs text-slate-500 mb-1">可执行文件</div>
                        <div className="font-mono text-xs text-slate-700 bg-slate-50 p-2 rounded border border-slate-100 break-all">
                          {geminiInfo.executable_path ?? "—"}
                        </div>
                      </div>
                      <div>
                        <div className="text-xs text-slate-500 mb-1">版本</div>
                        <div className="font-mono text-xs text-slate-700 bg-slate-50 p-2 rounded border border-slate-100 break-all">
                          {geminiInfo.version ?? "—"}
                        </div>
                      </div>
                    </div>
                  </div>

                  <div className="space-y-4">
                    <h3 className="text-sm font-semibold text-slate-900 flex items-center gap-2">
                      <FileJson className="h-4 w-4 text-slate-400" />
                      解析环境
                    </h3>
                    <div className="space-y-3">
                      <div>
                        <div className="text-xs text-slate-500 mb-1">$SHELL</div>
                        <div className="font-mono text-xs text-slate-700 bg-slate-50 p-2 rounded border border-slate-100 break-all">
                          {geminiInfo.shell ?? "—"}
                        </div>
                      </div>
                      <div>
                        <div className="text-xs text-slate-500 mb-1">解析方式</div>
                        <div className="font-mono text-xs text-slate-700 bg-slate-50 p-2 rounded border border-slate-100 break-all">
                          {geminiInfo.resolved_via}
                        </div>
                      </div>
                    </div>
                  </div>
                </div>
              )}

              {geminiInfo?.error && (
                <div className="mt-4 rounded-lg bg-rose-50 p-4 text-sm text-rose-600 flex items-start gap-2">
                  <AlertTriangle className="h-5 w-5 shrink-0" />
                  <div>
                    <span className="font-semibold">检测失败：</span>
                    {geminiInfo.error}
                  </div>
                </div>
              )}
            </Card>
          </div>
        )}
      </div>
    </div>
  );
}
