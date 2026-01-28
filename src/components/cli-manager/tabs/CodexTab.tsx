import { useEffect, useMemo, useState, type ReactNode } from "react";
import { toast } from "sonner";
import type {
  CodexConfigPatch,
  CodexConfigState,
  SimpleCliInfo,
} from "../../../services/cliManager";
import { cn } from "../../../utils/cn";
import { Button } from "../../../ui/Button";
import { Card } from "../../../ui/Card";
import { Input } from "../../../ui/Input";
import { Select } from "../../../ui/Select";
import { Switch } from "../../../ui/Switch";
import {
  AlertTriangle,
  CheckCircle2,
  ExternalLink,
  FileJson,
  FolderOpen,
  RefreshCw,
  Terminal,
  Settings,
} from "lucide-react";

export type CliManagerAvailability = "checking" | "available" | "unavailable";

export type CliManagerCodexTabProps = {
  codexAvailable: CliManagerAvailability;
  codexLoading: boolean;
  codexConfigLoading: boolean;
  codexConfigSaving: boolean;
  codexInfo: SimpleCliInfo | null;
  codexConfig: CodexConfigState | null;
  refreshCodex: () => Promise<void> | void;
  openCodexConfigDir: () => Promise<void> | void;
  persistCodexConfig: (patch: CodexConfigPatch) => Promise<void> | void;
};

function SettingItem({
  label,
  subtitle,
  children,
  className,
}: {
  label: string;
  subtitle: string;
  children: ReactNode;
  className?: string;
}) {
  return (
    <div
      className={cn(
        "flex flex-col gap-2 py-3 sm:flex-row sm:items-start sm:justify-between",
        className
      )}
    >
      <div className="min-w-0">
        <div className="text-sm text-slate-700">{label}</div>
        <div className="mt-1 text-xs text-slate-500 leading-relaxed">{subtitle}</div>
      </div>
      <div className="flex flex-wrap items-center justify-end gap-2">{children}</div>
    </div>
  );
}

function boolOrDefault(value: boolean | null, fallback: boolean) {
  return value ?? fallback;
}

function enumOrDefault(value: string | null, fallback: string) {
  return (value ?? fallback).trim();
}

export function CliManagerCodexTab({
  codexAvailable,
  codexLoading,
  codexConfigLoading,
  codexConfigSaving,
  codexInfo,
  codexConfig,
  refreshCodex,
  openCodexConfigDir,
  persistCodexConfig,
}: CliManagerCodexTabProps) {
  const [modelText, setModelText] = useState("");
  const [historyMaxBytesText, setHistoryMaxBytesText] = useState("");
  const [sandboxModeText, setSandboxModeText] = useState("");

  useEffect(() => {
    if (!codexConfig) return;
    setModelText(codexConfig.model ?? "");
    setHistoryMaxBytesText(
      codexConfig.history_max_bytes == null ? "" : String(codexConfig.history_max_bytes)
    );
    setSandboxModeText(codexConfig.sandbox_mode ?? "");
  }, [codexConfig]);

  const saving = codexConfigSaving;
  const loading = codexLoading || codexConfigLoading;

  useEffect(() => {
    if (!codexConfig) return;
    if (saving) return;
    setSandboxModeText(codexConfig.sandbox_mode ?? "");
  }, [saving, codexConfig?.sandbox_mode, codexConfig]);

  const defaults = useMemo(() => {
    return {
      sandbox_mode: "workspace-write",
    };
  }, []);

  const effectiveSandboxMode = useMemo(() => {
    return enumOrDefault(sandboxModeText.trim() || null, defaults.sandbox_mode);
  }, [sandboxModeText, defaults.sandbox_mode]);

  return (
    <div className="space-y-6">
      <Card className="overflow-hidden">
        <div className="border-b border-slate-100">
          <div className="flex flex-col gap-4 p-6">
            <div className="flex flex-col md:flex-row items-start md:items-center justify-between gap-4">
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
                    ) : codexAvailable === "checking" || loading ? (
                      <span className="inline-flex items-center gap-1.5 rounded-full bg-blue-50 px-2.5 py-0.5 text-xs font-medium text-blue-700 ring-1 ring-inset ring-blue-600/20">
                        <RefreshCw className="h-3 w-3 animate-spin" />
                        加载中...
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
                onClick={() => void refreshCodex()}
                variant="secondary"
                size="sm"
                disabled={loading}
                className="gap-2"
              >
                <RefreshCw className={cn("h-3.5 w-3.5", loading && "animate-spin")} />
                刷新
              </Button>
            </div>

            {codexConfig && (
              <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-3 mt-2">
                <div className="bg-slate-50 rounded-lg p-3 border border-slate-100">
                  <div className="flex items-center gap-1.5 text-xs text-slate-500 mb-1.5">
                    <FolderOpen className="h-3 w-3" />
                    CODEX_HOME
                  </div>
                  <div className="flex items-center gap-1.5">
                    <div
                      className="font-mono text-xs text-slate-700 truncate flex-1"
                      title={codexConfig.config_dir}
                    >
                      {codexConfig.config_dir}
                    </div>
                    <Button
                      onClick={() => void openCodexConfigDir()}
                      disabled={!codexConfig.can_open_config_dir}
                      size="sm"
                      variant="ghost"
                      className="shrink-0 h-6 w-6 p-0 hover:bg-slate-200"
                      title={
                        codexConfig.can_open_config_dir
                          ? "打开配置目录"
                          : "受权限限制，无法自动打开（仅允许 $HOME/.codex 下的路径）"
                      }
                    >
                      <ExternalLink className="h-3 w-3" />
                    </Button>
                  </div>
                  {!codexConfig.can_open_config_dir ? (
                    <div className="mt-1 text-[11px] text-amber-700">
                      受权限限制，应用仅允许打开 $HOME/.codex 下的目录；请手动打开该路径。
                    </div>
                  ) : null}
                </div>

                <div className="bg-slate-50 rounded-lg p-3 border border-slate-100">
                  <div className="flex items-center gap-1.5 text-xs text-slate-500 mb-1.5">
                    <FileJson className="h-3 w-3" />
                    config.toml
                  </div>
                  <div
                    className="font-mono text-xs text-slate-700 truncate"
                    title={codexConfig.config_path}
                  >
                    {codexConfig.config_path}
                  </div>
                  <div className="mt-1 text-[11px] text-slate-500">
                    {codexConfig.exists ? "已存在" : "不存在（将自动创建）"}
                  </div>
                </div>

                <div className="bg-slate-50 rounded-lg p-3 border border-slate-100">
                  <div className="flex items-center gap-1.5 text-xs text-slate-500 mb-1.5">
                    <Terminal className="h-3 w-3" />
                    可执行文件
                  </div>
                  <div
                    className="font-mono text-xs text-slate-700 truncate"
                    title={codexInfo?.executable_path ?? "—"}
                  >
                    {codexInfo?.executable_path ?? "—"}
                  </div>
                </div>

                <div className="bg-slate-50 rounded-lg p-3 border border-slate-100">
                  <div className="flex items-center gap-1.5 text-xs text-slate-500 mb-1.5">
                    <Settings className="h-3 w-3" />
                    解析方式
                  </div>
                  <div
                    className="font-mono text-xs text-slate-700 truncate"
                    title={codexInfo?.resolved_via ?? "—"}
                  >
                    {codexInfo?.resolved_via ?? "—"}
                  </div>
                  <div className="mt-1 text-[11px] text-slate-500">
                    SHELL: {codexInfo?.shell ?? "—"}
                  </div>
                </div>
              </div>
            )}

            <div className="text-xs text-slate-500">
              注意：Codex 还会读取 Team Config（例如 repo 内 `.codex/`），其优先级可能高于
              `$CODEX_HOME`。
            </div>
          </div>
        </div>

        {codexAvailable === "unavailable" ? (
          <div className="text-sm text-slate-600 text-center py-8">仅在 Tauri Desktop 环境可用</div>
        ) : !codexConfig ? (
          <div className="text-sm text-slate-500 text-center py-8">暂无配置，请尝试刷新</div>
        ) : (
          <div className="p-6 space-y-6">
            <div className="rounded-lg border border-slate-200 bg-white p-5">
              <h3 className="text-sm font-semibold text-slate-900 flex items-center gap-2 mb-3">
                <Settings className="h-4 w-4 text-slate-400" />
                基础配置
              </h3>
              <div className="divide-y divide-slate-100">
                <SettingItem
                  label="默认模型 (model)"
                  subtitle="设置 Codex 默认使用的模型（例如 gpt-5-codex）。留空表示不设置（交由 Codex 默认/上层配置决定）。"
                >
                  <Input
                    value={modelText}
                    onChange={(e) => setModelText(e.currentTarget.value)}
                    onBlur={() => void persistCodexConfig({ model: modelText.trim() })}
                    placeholder="例如：gpt-5-codex"
                    className="font-mono w-[280px] max-w-full"
                    disabled={saving}
                  />
                </SettingItem>

                <SettingItem
                  label="审批策略 (approval_policy)"
                  subtitle="控制何时需要你确认才会执行命令。推荐 on-request（默认）或 on-failure。"
                >
                  <Select
                    value={codexConfig.approval_policy ?? ""}
                    onChange={(e) =>
                      void persistCodexConfig({ approval_policy: e.currentTarget.value })
                    }
                    disabled={saving}
                    className="w-[220px] max-w-full font-mono"
                  >
                    <option value="">默认（不设置）</option>
                    <option value="untrusted">不信任（untrusted）</option>
                    <option value="on-failure">失败时（on-failure）</option>
                    <option value="on-request">请求时（on-request）</option>
                    <option value="never">从不询问（never）</option>
                  </Select>
                </SettingItem>

                <SettingItem
                  label="沙箱模式 (sandbox_mode)"
                  subtitle="控制文件/网络访问策略。danger-full-access 风险极高，仅在完全信任的环境使用。"
                >
                  <Select
                    value={sandboxModeText}
                    onChange={(e) => {
                      const next = e.currentTarget.value;
                      if (next === "danger-full-access") {
                        const ok = window.confirm(
                          "你选择了 danger-full-access（危险：完全访问）。确认要继续吗？"
                        );
                        if (!ok) {
                          setSandboxModeText(codexConfig.sandbox_mode ?? "");
                          return;
                        }
                      }
                      setSandboxModeText(next);
                      void persistCodexConfig({ sandbox_mode: next });
                    }}
                    disabled={saving}
                    className="w-[220px] max-w-full font-mono"
                  >
                    <option value="">默认（不设置）</option>
                    <option value="read-only">只读（read-only）</option>
                    <option value="workspace-write">工作区写入（workspace-write）</option>
                    <option value="danger-full-access">危险：完全访问（danger-full-access）</option>
                  </Select>
                </SettingItem>

                <SettingItem
                  label="推理强度 (model_reasoning_effort)"
                  subtitle="调整推理强度（仅对支持的模型/Responses API 生效）。值越高通常越稳健但更慢。"
                >
                  <Select
                    value={codexConfig.model_reasoning_effort ?? ""}
                    onChange={(e) =>
                      void persistCodexConfig({ model_reasoning_effort: e.currentTarget.value })
                    }
                    disabled={saving}
                    className="w-[220px] max-w-full font-mono"
                  >
                    <option value="">默认（不设置）</option>
                    <option value="minimal">最低（minimal）</option>
                    <option value="low">低（low）</option>
                    <option value="medium">中等（medium）</option>
                    <option value="high">高（high）</option>
                    <option value="xhigh">极高（xhigh）</option>
                  </Select>
                </SettingItem>
              </div>
            </div>

            <div className="rounded-lg border border-slate-200 bg-white p-5">
              <h3 className="text-sm font-semibold text-slate-900 flex items-center gap-2 mb-3">
                <Settings className="h-4 w-4 text-slate-400" />
                交互与显示
              </h3>
              <div className="divide-y divide-slate-100">
                <SettingItem
                  label="文件打开方式 (file_opener)"
                  subtitle="控制点击 citations 时使用的编辑器/URI scheme（默认 vscode）。"
                >
                  <Select
                    value={codexConfig.file_opener ?? ""}
                    onChange={(e) =>
                      void persistCodexConfig({ file_opener: e.currentTarget.value })
                    }
                    disabled={saving}
                    className="w-[220px] max-w-full font-mono"
                  >
                    <option value="">默认（不设置）</option>
                    <option value="vscode">vscode</option>
                    <option value="vscode-insiders">vscode-insiders</option>
                    <option value="windsurf">windsurf</option>
                    <option value="cursor">cursor</option>
                    <option value="none">无（none）</option>
                  </Select>
                </SettingItem>

                <SettingItem
                  label="隐藏推理事件 (hide_agent_reasoning)"
                  subtitle="开启写入 hide_agent_reasoning=true；关闭删除该项（不写 false）。"
                >
                  <Switch
                    checked={boolOrDefault(codexConfig.hide_agent_reasoning, false)}
                    onCheckedChange={(checked) =>
                      void persistCodexConfig({ hide_agent_reasoning: checked })
                    }
                    disabled={saving}
                  />
                </SettingItem>

                <SettingItem
                  label="显示原始推理 (show_raw_agent_reasoning)"
                  subtitle="开启写入 show_raw_agent_reasoning=true；关闭删除该项（不写 false）。"
                >
                  <Switch
                    checked={boolOrDefault(codexConfig.show_raw_agent_reasoning, false)}
                    onCheckedChange={(checked) =>
                      void persistCodexConfig({ show_raw_agent_reasoning: checked })
                    }
                    disabled={saving}
                  />
                </SettingItem>

                <SettingItem
                  label="历史记录持久化 (history.persistence)"
                  subtitle="控制是否将会话记录写入 history.jsonl（默认 save-all）。"
                >
                  <Select
                    value={codexConfig.history_persistence ?? ""}
                    onChange={(e) =>
                      void persistCodexConfig({ history_persistence: e.currentTarget.value })
                    }
                    disabled={saving}
                    className="w-[220px] max-w-full font-mono"
                  >
                    <option value="">默认（不设置）</option>
                    <option value="save-all">全部保存（save-all）</option>
                    <option value="none">不保存（none）</option>
                  </Select>
                </SettingItem>

                <SettingItem
                  label="历史大小上限 (history.max_bytes)"
                  subtitle="限制 history.jsonl 最大大小（字节）。留空或 0 表示不设置该项。"
                >
                  <Input
                    type="number"
                    value={historyMaxBytesText}
                    onChange={(e) => setHistoryMaxBytesText(e.currentTarget.value)}
                    onBlur={() => {
                      const trimmed = historyMaxBytesText.trim();
                      if (!trimmed) {
                        void persistCodexConfig({ history_max_bytes: 0 });
                        return;
                      }
                      const n = Math.floor(Number(trimmed));
                      if (!Number.isFinite(n) || n < 0) {
                        toast("history.max_bytes 必须为非负整数");
                        setHistoryMaxBytesText(
                          codexConfig.history_max_bytes == null
                            ? ""
                            : String(codexConfig.history_max_bytes)
                        );
                        return;
                      }
                      void persistCodexConfig({ history_max_bytes: n });
                    }}
                    className="font-mono w-[220px] max-w-full"
                    min={0}
                    disabled={saving}
                    placeholder="默认"
                  />
                </SettingItem>
              </div>
            </div>

            <div className="rounded-lg border border-slate-200 bg-white p-5">
              <h3 className="text-sm font-semibold text-slate-900 flex items-center gap-2 mb-3">
                <Settings className="h-4 w-4 text-slate-400" />
                Sandbox（workspace-write）
              </h3>
              <div className="divide-y divide-slate-100">
                <SettingItem
                  label="允许联网 (sandbox_workspace_write.network_access)"
                  subtitle="仅在 sandbox_mode=workspace-write 时生效。开启写入 network_access=true；关闭删除该项（不写 false）。"
                >
                  <Switch
                    checked={boolOrDefault(
                      codexConfig.sandbox_workspace_write_network_access,
                      false
                    )}
                    onCheckedChange={(checked) =>
                      void persistCodexConfig({ sandbox_workspace_write_network_access: checked })
                    }
                    disabled={saving}
                  />
                </SettingItem>
              </div>
              {effectiveSandboxMode !== "workspace-write" ? (
                <div className="mt-3 rounded-lg bg-amber-50 p-3 text-xs text-amber-700 flex items-start gap-2">
                  <AlertTriangle className="h-4 w-4 shrink-0 mt-0.5" />
                  <div>
                    当前 sandbox_mode 不是 <span className="font-mono">workspace-write</span>
                    ，此分区设置可能不会生效。
                  </div>
                </div>
              ) : null}
            </div>

            <div className="rounded-lg border border-slate-200 bg-white p-5">
              <h3 className="text-sm font-semibold text-slate-900 flex items-center gap-2 mb-3">
                <Settings className="h-4 w-4 text-slate-400" />
                TUI（布局与样式）
              </h3>
              <div className="divide-y divide-slate-100">
                <SettingItem
                  label="动画效果 (tui.animations)"
                  subtitle="开启写入 tui.animations=true；关闭删除该项（不写 false）。"
                >
                  <Switch
                    checked={boolOrDefault(codexConfig.tui_animations, false)}
                    onCheckedChange={(checked) =>
                      void persistCodexConfig({ tui_animations: checked })
                    }
                    disabled={saving}
                  />
                </SettingItem>

                <SettingItem
                  label="备用屏幕 (tui.alternate_screen)"
                  subtitle="控制是否使用 alternate screen（never 可保留终端滚动回溯；默认 auto）。"
                >
                  <Select
                    value={codexConfig.tui_alternate_screen ?? ""}
                    onChange={(e) =>
                      void persistCodexConfig({ tui_alternate_screen: e.currentTarget.value })
                    }
                    disabled={saving}
                    className="w-[220px] max-w-full font-mono"
                  >
                    <option value="">默认（不设置）</option>
                    <option value="auto">自动（auto）</option>
                    <option value="always">始终（always）</option>
                    <option value="never">从不（never）</option>
                  </Select>
                </SettingItem>

                <SettingItem
                  label="显示新手提示 (tui.show_tooltips)"
                  subtitle="开启写入 tui.show_tooltips=true；关闭删除该项（不写 false）。"
                >
                  <Switch
                    checked={boolOrDefault(codexConfig.tui_show_tooltips, false)}
                    onCheckedChange={(checked) =>
                      void persistCodexConfig({ tui_show_tooltips: checked })
                    }
                    disabled={saving}
                  />
                </SettingItem>

                <SettingItem
                  label="滚轮方向反转 (tui.scroll_invert)"
                  subtitle="开启写入 tui.scroll_invert=true；关闭删除该项（不写 false）。"
                >
                  <Switch
                    checked={boolOrDefault(codexConfig.tui_scroll_invert, false)}
                    onCheckedChange={(checked) =>
                      void persistCodexConfig({ tui_scroll_invert: checked })
                    }
                    disabled={saving}
                  />
                </SettingItem>
              </div>
            </div>

            <div className="rounded-lg border border-slate-200 bg-white p-5">
              <h3 className="text-sm font-semibold text-slate-900 flex items-center gap-2 mb-3">
                <Settings className="h-4 w-4 text-slate-400" />
                Features（实验/可选能力）
              </h3>
              <div className="divide-y divide-slate-100">
                <SettingItem
                  label="shell_snapshot"
                  subtitle="测试版：快照 shell 环境以加速重复命令。开启写入 shell_snapshot=true；"
                >
                  <Switch
                    checked={boolOrDefault(codexConfig.features_shell_snapshot, false)}
                    onCheckedChange={(checked) =>
                      void persistCodexConfig({ features_shell_snapshot: checked })
                    }
                    disabled={saving}
                  />
                </SettingItem>

                <SettingItem
                  label="web_search_request"
                  subtitle="稳定：允许模型发起 Web Search 请求。开启写入 web_search_request=true；"
                >
                  <Switch
                    checked={boolOrDefault(codexConfig.features_web_search_request, false)}
                    onCheckedChange={(checked) =>
                      void persistCodexConfig({ features_web_search_request: checked })
                    }
                    disabled={saving}
                  />
                </SettingItem>

                <SettingItem
                  label="unified_exec"
                  subtitle="测试版：使用统一的、基于 PTY 的 exec 工具。开启写入 unified_exec=true；"
                >
                  <Switch
                    checked={boolOrDefault(codexConfig.features_unified_exec, false)}
                    onCheckedChange={(checked) =>
                      void persistCodexConfig({ features_unified_exec: checked })
                    }
                    disabled={saving}
                  />
                </SettingItem>

                <SettingItem
                  label="shell_tool"
                  subtitle="稳定：启用默认 shell 工具。开启写入 shell_tool=true；"
                >
                  <Switch
                    checked={boolOrDefault(codexConfig.features_shell_tool, false)}
                    onCheckedChange={(checked) =>
                      void persistCodexConfig({ features_shell_tool: checked })
                    }
                    disabled={saving}
                  />
                </SettingItem>

                <SettingItem
                  label="exec_policy"
                  subtitle="实验性：对 shell/unified_exec 强制执行规则检查。开启写入 exec_policy=true；"
                >
                  <Switch
                    checked={boolOrDefault(codexConfig.features_exec_policy, false)}
                    onCheckedChange={(checked) =>
                      void persistCodexConfig({ features_exec_policy: checked })
                    }
                    disabled={saving}
                  />
                </SettingItem>

                <SettingItem
                  label="apply_patch_freeform"
                  subtitle="实验性：启用自由格式 apply_patch 工具。开启写入 apply_patch_freeform=true；"
                >
                  <Switch
                    checked={boolOrDefault(codexConfig.features_apply_patch_freeform, false)}
                    onCheckedChange={(checked) =>
                      void persistCodexConfig({ features_apply_patch_freeform: checked })
                    }
                    disabled={saving}
                  />
                </SettingItem>

                <SettingItem
                  label="remote_compaction"
                  subtitle="实验性：启用 remote compaction（需要 ChatGPT 身份验证）。开启写入 remote_compaction=true；"
                >
                  <Switch
                    checked={boolOrDefault(codexConfig.features_remote_compaction, false)}
                    onCheckedChange={(checked) =>
                      void persistCodexConfig({ features_remote_compaction: checked })
                    }
                    disabled={saving}
                  />
                </SettingItem>

                <SettingItem
                  label="remote_models"
                  subtitle="实验性：启动时刷新远程模型列表。开启写入 remote_models=true；"
                >
                  <Switch
                    checked={boolOrDefault(codexConfig.features_remote_models, false)}
                    onCheckedChange={(checked) =>
                      void persistCodexConfig({ features_remote_models: checked })
                    }
                    disabled={saving}
                  />
                </SettingItem>

                <SettingItem
                  label="powershell_utf8"
                  subtitle="Windows：强制 PowerShell 使用 UTF-8 输出。开启写入 powershell_utf8=true；"
                >
                  <Switch
                    checked={boolOrDefault(codexConfig.features_powershell_utf8, false)}
                    onCheckedChange={(checked) =>
                      void persistCodexConfig({ features_powershell_utf8: checked })
                    }
                    disabled={saving}
                  />
                </SettingItem>

                <SettingItem
                  label="child_agents_md"
                  subtitle="实验性：即使没有 AGENTS.md 也附加其作用域/优先级说明。开启写入 child_agents_md=true；"
                >
                  <Switch
                    checked={boolOrDefault(codexConfig.features_child_agents_md, false)}
                    onCheckedChange={(checked) =>
                      void persistCodexConfig({ features_child_agents_md: checked })
                    }
                    disabled={saving}
                  />
                </SettingItem>

                <SettingItem
                  label="experimental_windows_sandbox"
                  subtitle="实验性：Windows 受限令牌 sandbox。开启写入 experimental_windows_sandbox=true；"
                >
                  <Switch
                    checked={boolOrDefault(
                      codexConfig.features_experimental_windows_sandbox,
                      false
                    )}
                    onCheckedChange={(checked) =>
                      void persistCodexConfig({ features_experimental_windows_sandbox: checked })
                    }
                    disabled={saving}
                  />
                </SettingItem>

                <SettingItem
                  label="elevated_windows_sandbox"
                  subtitle="实验性：Windows 提权 sandbox 流程。开启写入 elevated_windows_sandbox=true；"
                >
                  <Switch
                    checked={boolOrDefault(codexConfig.features_elevated_windows_sandbox, false)}
                    onCheckedChange={(checked) =>
                      void persistCodexConfig({ features_elevated_windows_sandbox: checked })
                    }
                    disabled={saving}
                  />
                </SettingItem>

                <SettingItem
                  label="collab"
                  subtitle="Beta：启用多代理协作，允许代理间通过 spawn_agent/send_input/wait/close 工具协调工作。"
                >
                  <Switch
                    checked={boolOrDefault(codexConfig.features_collab, false)}
                    onCheckedChange={(checked) =>
                      void persistCodexConfig({ features_collab: checked })
                    }
                    disabled={saving}
                  />
                </SettingItem>

                <SettingItem
                  label="collaboration_modes"
                  subtitle="Beta：启用协作模式预设，TUI 中提供 Coding/Plan 模式选择与规划-执行阶段自动切换。"
                >
                  <Switch
                    checked={boolOrDefault(codexConfig.features_collaboration_modes, false)}
                    onCheckedChange={(checked) =>
                      void persistCodexConfig({ features_collaboration_modes: checked })
                    }
                    disabled={saving}
                  />
                </SettingItem>
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
  );
}
