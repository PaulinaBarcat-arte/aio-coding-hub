import { useEffect, useMemo, useState } from "react";
import { Terminal, Globe, Edit2, Trash2, Command, Link } from "lucide-react";
import { toast } from "sonner";
import { logToConsole } from "../services/consoleLog";
import {
  mcpImportServers,
  mcpParseJson,
  mcpServerDelete,
  mcpServerSetEnabled,
  mcpServerUpsert,
  mcpServersList,
  type McpImportServer,
  type McpServerSummary,
  type McpTransport,
} from "../services/mcp";
import type { CliKey } from "../services/providers";
import { Button } from "../ui/Button";
import { Card } from "../ui/Card";
import { Dialog } from "../ui/Dialog";
import { Switch } from "../ui/Switch";
import { cn } from "../utils/cn";
import { formatUnknownError } from "../utils/errors";

type CliItem = { key: CliKey; name: string; file: string };

const CLIS: CliItem[] = [
  { key: "claude", name: "Claude Code", file: "~/.claude.json" },
  { key: "codex", name: "Codex", file: "~/.codex/config.toml" },
  { key: "gemini", name: "Gemini", file: "~/.gemini/settings.json" },
];

function parseLines(text: string) {
  return text
    .split("\n")
    .map((l) => l.trim())
    .filter(Boolean);
}

function parseKeyValueLines(text: string, hint: string) {
  const out: Record<string, string> = {};
  const lines = parseLines(text);
  for (const line of lines) {
    const idx = line.indexOf("=");
    if (idx <= 0) {
      throw new Error(`${hint} 格式错误：请使用 KEY=VALUE（示例：FOO=bar）`);
    }
    const k = line.slice(0, idx).trim();
    const v = line.slice(idx + 1).trim();
    if (!k) throw new Error(`${hint} 格式错误：KEY 不能为空`);
    out[k] = v;
  }
  return out;
}

type ImportRowStatus = "insert" | "update" | "duplicate" | "invalid";
type ImportRow = {
  server: McpImportServer;
  status: ImportRowStatus;
  reason?: string;
  norm_name: string;
};

type ImportParseErrorView = {
  title: string;
  summary: string;
  details: Array<{ label: string; value: string; mono?: boolean }>;
  hint?: string;
  raw: string;
};

function normalizeName(name: string) {
  return name.trim().toLowerCase();
}

function describeServer(server: Pick<McpServerSummary, "transport" | "command" | "url">) {
  if (server.transport === "http") return server.url || "（未填写 url）";
  return server.command || "（未填写 command）";
}

function isCliKey(cliKey: string): cliKey is CliKey {
  return cliKey === "claude" || cliKey === "codex" || cliKey === "gemini";
}

function labelCli(cliKey: string | null | undefined) {
  if (!cliKey) return null;
  if (!isCliKey(cliKey)) return cliKey;
  const found = CLIS.find((c) => c.key === cliKey);
  return found ? found.name : cliKey;
}

function buildImportParseErrorView(raw: string): ImportParseErrorView {
  const trimmed = raw.trim();
  const msg = trimmed.startsWith("Error: ") ? trimmed.slice("Error: ".length) : trimmed;
  const details: ImportParseErrorView["details"] = [];

  const missingFieldMatch = msg.match(
    /SEC_INVALID_INPUT:\s*import\s+(\w+)\s+server\s+['"](.+?)['"]\s+missing\s+(command|url)\s*$/i
  );
  if (missingFieldMatch) {
    const cliKey = missingFieldMatch[1];
    const serverName = missingFieldMatch[2];
    const field = missingFieldMatch[3].toLowerCase();
    const cliLabel = labelCli(cliKey) ?? cliKey;

    details.push({ label: "CLI", value: cliLabel });
    details.push({ label: "Server", value: serverName });
    details.push({ label: "缺失字段", value: field, mono: true });

    return {
      title: "JSON 字段缺失",
      summary: `导入配置缺少必填字段：${field}`,
      details,
      hint:
        field === "command"
          ? "STDIO 类型需要提供 command（本地启动命令）。请补齐后重新解析。"
          : "HTTP 类型需要提供 url（远程服务地址）。请补齐后重新解析。",
      raw: msg,
    };
  }

  const conflictMatch = msg.match(
    /SEC_INVALID_INPUT:\s*import conflict for server\s+['"](.+?)['"]\s+across platforms\s*$/i
  );
  if (conflictMatch) {
    const serverName = conflictMatch[1];
    details.push({ label: "Server", value: serverName });
    return {
      title: "导入冲突",
      summary: "同名 MCP Server 在多个 CLI 段落中定义不一致（无法合并）。",
      details,
      hint: "请确保同名 server 的 transport/command/url/args 等配置一致后再导入。",
      raw: msg,
    };
  }

  const invalidJsonMatch = msg.match(/SEC_INVALID_INPUT:\s*invalid JSON:\s*(.*)$/i);
  if (invalidJsonMatch) {
    const reason = invalidJsonMatch[1]?.trim();
    if (reason) details.push({ label: "详情", value: reason });
    return {
      title: "JSON 语法错误",
      summary: "无法解析 JSON，请检查格式（逗号、引号、括号）以及行列号提示。",
      details,
      raw: msg,
    };
  }

  if (/SEC_INVALID_INPUT:\s*unsupported JSON shape/i.test(msg)) {
    return {
      title: "不支持的 JSON 结构",
      summary: "当前仅支持 code-switch-R 的 mcp.json 结构或数组格式。",
      details: [],
      hint: "请使用数组格式（[{name,transport,command,url,args,env,cwd,headers,...}]）或粘贴 code-switch-R 兼容结构后重试。",
      raw: msg,
    };
  }

  if (/SEC_INVALID_INPUT:\s*JSON is required/i.test(msg)) {
    return {
      title: "缺少 JSON",
      summary: "请先粘贴要导入的 JSON，再点击「解析预览」。",
      details: [],
      raw: msg,
    };
  }

  return {
    title: "解析失败",
    summary: msg || "未知错误",
    details: [],
    raw: msg || trimmed,
  };
}

function enabledLabel(server: McpServerSummary) {
  const enabled: string[] = [];
  if (server.enabled_claude) enabled.push("Claude");
  if (server.enabled_codex) enabled.push("Codex");
  if (server.enabled_gemini) enabled.push("Gemini");
  return enabled.length ? enabled.join(" / ") : "未启用";
}

function enabledLabelFromImport(
  server: Pick<McpImportServer, "enabled_claude" | "enabled_codex" | "enabled_gemini">
) {
  const enabled: string[] = [];
  if (server.enabled_claude) enabled.push("Claude");
  if (server.enabled_codex) enabled.push("Codex");
  if (server.enabled_gemini) enabled.push("Gemini");
  return enabled.length ? enabled.join(" / ") : "未启用";
}

export function McpPage() {
  const [items, setItems] = useState<McpServerSummary[]>([]);
  const [loading, setLoading] = useState(false);
  const [saving, setSaving] = useState(false);
  const [toggling, setToggling] = useState(false);

  const [dialogOpen, setDialogOpen] = useState(false);
  const [editTarget, setEditTarget] = useState<McpServerSummary | null>(null);

  const [name, setName] = useState("");
  const [transport, setTransport] = useState<McpTransport>("stdio");
  const [command, setCommand] = useState("");
  const [argsText, setArgsText] = useState("");
  const [envText, setEnvText] = useState("");
  const [cwd, setCwd] = useState("");
  const [url, setUrl] = useState("");
  const [headersText, setHeadersText] = useState("");

  const [enabledClaude, setEnabledClaude] = useState(false);
  const [enabledCodex, setEnabledCodex] = useState(false);
  const [enabledGemini, setEnabledGemini] = useState(false);

  const [fillDialogOpen, setFillDialogOpen] = useState(false);
  const [fillText, setFillText] = useState("");
  const [fillParsing, setFillParsing] = useState(false);
  const [fillOptions, setFillOptions] = useState<McpImportServer[] | null>(null);
  const [fillIndex, setFillIndex] = useState(0);

  const [deleteTarget, setDeleteTarget] = useState<McpServerSummary | null>(null);

  const [importOpen, setImportOpen] = useState(false);
  const [importText, setImportText] = useState("");
  const [importParsing, setImportParsing] = useState(false);
  const [importing, setImporting] = useState(false);
  const [importPreview, setImportPreview] = useState<McpImportServer[] | null>(null);
  const [importParseError, setImportParseError] = useState<string | null>(null);

  async function refresh() {
    setLoading(true);
    try {
      const next = await mcpServersList();
      if (!next) {
        setItems([]);
        return;
      }
      setItems(next);
    } catch (err) {
      logToConsole("error", "加载 MCP Servers 失败", { error: String(err) });
      toast("加载失败：请查看控制台日志");
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => {
    void refresh();
  }, []);

  useEffect(() => {
    if (!dialogOpen) return;
    if (editTarget) {
      setName(editTarget.name);
      setTransport(editTarget.transport);
      setCommand(editTarget.command ?? "");
      setArgsText((editTarget.args ?? []).join("\n"));
      setEnvText(
        Object.entries(editTarget.env ?? {})
          .map(([k, v]) => `${k}=${v}`)
          .join("\n")
      );
      setCwd(editTarget.cwd ?? "");
      setUrl(editTarget.url ?? "");
      setHeadersText(
        Object.entries(editTarget.headers ?? {})
          .map(([k, v]) => `${k}=${v}`)
          .join("\n")
      );
      setEnabledClaude(editTarget.enabled_claude);
      setEnabledCodex(editTarget.enabled_codex);
      setEnabledGemini(editTarget.enabled_gemini);
      setFillText("");
      setFillOptions(null);
      setFillIndex(0);
      return;
    }

    setName("");
    setTransport("stdio");
    setCommand("");
    setArgsText("");
    setEnvText("");
    setCwd("");
    setUrl("");
    setHeadersText("");
    setEnabledClaude(false);
    setEnabledCodex(false);
    setEnabledGemini(false);
    setFillText("");
    setFillOptions(null);
    setFillIndex(0);
  }, [dialogOpen, editTarget]);

  const transportHint = useMemo(() => {
    return transport === "http" ? "HTTP（远程服务）" : "STDIO（本地命令）";
  }, [transport]);

  const importAnalysis = useMemo(() => {
    if (!importPreview) return null;

    const existingByNormName = new Map<string, McpServerSummary>();
    for (const item of items) {
      const norm = normalizeName(item.name);
      if (!norm) continue;
      if (!existingByNormName.has(norm)) existingByNormName.set(norm, item);
    }

    const base = importPreview.map((server) => {
      const norm = normalizeName(server.name ?? "");
      let reason: string | undefined;
      if (!norm) reason = "名称不能为空";
      else if (server.transport === "stdio" && !server.command?.trim())
        reason = "STDIO 类型必须填写 command";
      else if (server.transport === "http" && !server.url?.trim()) reason = "HTTP 类型必须填写 url";

      return { server, norm, reason };
    });

    const lastIndexByNorm = new Map<string, number>();
    base.forEach((row, idx) => {
      if (!row.norm) return;
      lastIndexByNorm.set(row.norm, idx);
    });

    const rows: ImportRow[] = [];
    const insert_names: string[] = [];
    const update_names: string[] = [];
    const skipped_duplicates: string[] = [];
    const skipped_invalid: Array<{ name: string; reason: string }> = [];

    let insert = 0;
    let update = 0;
    let duplicate = 0;
    let invalid = 0;

    for (let idx = 0; idx < base.length; idx += 1) {
      const row = base[idx];
      const displayName = row.server.name || "（未命名）";

      const lastIdx = row.norm ? lastIndexByNorm.get(row.norm) : undefined;
      if (row.norm && lastIdx !== undefined && lastIdx !== idx) {
        duplicate += 1;
        skipped_duplicates.push(displayName);
        rows.push({
          server: row.server,
          status: "duplicate",
          reason: "同名被后者覆盖（last-wins）",
          norm_name: row.norm,
        });
        continue;
      }

      if (row.reason) {
        invalid += 1;
        skipped_invalid.push({ name: displayName, reason: row.reason });
        rows.push({
          server: row.server,
          status: "invalid",
          reason: row.reason,
          norm_name: row.norm,
        });
        continue;
      }

      const isUpdate = existingByNormName.has(row.norm);
      if (isUpdate) {
        update += 1;
        update_names.push(displayName);
        rows.push({
          server: row.server,
          status: "update",
          norm_name: row.norm,
        });
      } else {
        insert += 1;
        insert_names.push(displayName);
        rows.push({
          server: row.server,
          status: "insert",
          norm_name: row.norm,
        });
      }
    }

    const effective_servers = rows
      .filter((r) => r.status === "insert" || r.status === "update")
      .map((r) => r.server);

    return {
      rows,
      summary: {
        total: rows.length,
        insert,
        update,
        duplicate,
        invalid,
        effective: effective_servers.length,
      },
      effective_servers,
      insert_names,
      update_names,
      skipped_duplicates,
      skipped_invalid,
    };
  }, [importPreview, items]);

  const importParseErrorView = useMemo(() => {
    if (!importParseError) return null;
    return buildImportParseErrorView(importParseError);
  }, [importParseError]);

  function applyImportedServer(server: McpImportServer) {
    setName(server.name ?? "");
    setTransport(server.transport);
    setCommand(server.command ?? "");
    setArgsText((server.args ?? []).join("\n"));
    setEnvText(
      Object.entries(server.env ?? {})
        .map(([k, v]) => `${k}=${v}`)
        .join("\n")
    );
    setCwd(server.cwd ?? "");
    setUrl(server.url ?? "");
    setHeadersText(
      Object.entries(server.headers ?? {})
        .map(([k, v]) => `${k}=${v}`)
        .join("\n")
    );
    setEnabledClaude(Boolean(server.enabled_claude));
    setEnabledCodex(Boolean(server.enabled_codex));
    setEnabledGemini(Boolean(server.enabled_gemini));
  }

  async function parseFillOptions() {
    if (fillParsing) return;
    if (!fillText.trim()) return;
    setFillParsing(true);
    try {
      const result = await mcpParseJson(fillText);
      if (!result) {
        toast("仅在 Tauri Desktop 环境可用");
        return;
      }

      const servers = result.servers ?? [];
      setFillOptions(servers);
      setFillIndex(0);

      if (servers.length === 0) {
        toast("解析成功，但未找到可回填的 MCP 配置");
        return;
      }
      logToConsole("info", "解析 MCP JSON（回填）", { count: servers.length });
      toast(`解析完成：${servers.length} 条`);
    } catch (err) {
      setFillOptions(null);
      logToConsole("error", "JSON 回填解析失败", { error: String(err) });
      toast(`解析失败：${String(err)}`);
    } finally {
      setFillParsing(false);
    }
  }

  function selectedFillServer() {
    if (!fillOptions || fillOptions.length === 0) return null;
    return fillOptions[fillIndex] ?? fillOptions[0] ?? null;
  }

  function applySelectedFillServer() {
    const server = selectedFillServer();
    if (!server) {
      toast("请先解析 JSON");
      return;
    }
    applyImportedServer(server);
    logToConsole("info", "从 JSON 回填 MCP 表单", {
      name: server.name,
      transport: server.transport,
    });
    toast(`已回填：${server.name}`);
    setFillDialogOpen(false);
  }

  async function importAndFill() {
    const server = selectedFillServer();
    if (server) {
      applySelectedFillServer();
      return;
    }

    if (fillParsing) return;
    if (!fillText.trim()) return;
    setFillParsing(true);
    try {
      const result = await mcpParseJson(fillText);
      if (!result) {
        toast("仅在 Tauri Desktop 环境可用");
        return;
      }

      const servers = result.servers ?? [];
      setFillOptions(servers);
      setFillIndex(0);

      if (servers.length === 0) {
        toast("解析成功，但未找到可回填的 MCP 配置");
        return;
      }

      applyImportedServer(servers[0]);
      logToConsole("info", "从 JSON 回填 MCP 表单", {
        name: servers[0].name,
        transport: servers[0].transport,
      });
      toast(`已回填：${servers[0].name}`);
      setFillDialogOpen(false);
    } catch (err) {
      setFillOptions(null);
      logToConsole("error", "JSON 回填解析失败", { error: String(err) });
      toast(`解析失败：${String(err)}`);
    } finally {
      setFillParsing(false);
    }
  }

  async function save() {
    if (saving) return;
    setSaving(true);
    try {
      const next = await mcpServerUpsert({
        server_id: editTarget?.id ?? null,
        // server_key 是内部标识，用于写入 CLI 配置文件：
        // - Claude/Gemini: JSON map key
        // - Codex: TOML table name
        // 为降低认知负担，创建时自动生成；编辑时保持不变。
        server_key: editTarget?.server_key ?? "",
        name,
        transport,
        command: transport === "stdio" ? command : null,
        args: transport === "stdio" ? parseLines(argsText) : [],
        env: transport === "stdio" ? parseKeyValueLines(envText, "Env") : {},
        cwd: transport === "stdio" ? (cwd.trim() ? cwd : null) : null,
        url: transport === "http" ? url : null,
        headers: transport === "http" ? parseKeyValueLines(headersText, "Headers") : {},
        enabled_claude: enabledClaude,
        enabled_codex: enabledCodex,
        enabled_gemini: enabledGemini,
      });

      if (!next) {
        toast("仅在 Tauri Desktop 环境可用");
        return;
      }

      logToConsole(
        editTarget ? "info" : "info",
        editTarget ? "更新 MCP Server" : "新增 MCP Server",
        {
          id: next.id,
          server_key: next.server_key,
          transport: next.transport,
          enabled: enabledLabel(next),
        }
      );

      toast(editTarget ? "已更新" : "已新增");
      setDialogOpen(false);
      setEditTarget(null);
      await refresh();
    } catch (err) {
      logToConsole("error", "保存 MCP Server 失败", { error: String(err) });
      toast(`保存失败：${String(err)}`);
    } finally {
      setSaving(false);
    }
  }

  async function toggleEnabled(server: McpServerSummary, cliKey: CliKey) {
    if (toggling) return;
    const current =
      cliKey === "claude"
        ? server.enabled_claude
        : cliKey === "codex"
          ? server.enabled_codex
          : server.enabled_gemini;
    const nextEnabled = !current;

    setToggling(true);
    try {
      const next = await mcpServerSetEnabled({
        server_id: server.id,
        cli_key: cliKey,
        enabled: nextEnabled,
      });
      if (!next) {
        toast("仅在 Tauri Desktop 环境可用");
        return;
      }

      setItems((prev) => prev.map((s) => (s.id === next.id ? next : s)));

      const cliLabel = CLIS.find((c) => c.key === cliKey)?.name ?? cliKey;
      logToConsole("info", "切换 MCP Server 生效范围", {
        id: next.id,
        server_key: next.server_key,
        cli: cliKey,
        enabled: nextEnabled,
      });
      toast(`${cliLabel}：${nextEnabled ? "已启用" : "已停用"}`);
    } catch (err) {
      logToConsole("error", "切换 MCP Server 生效范围失败", {
        error: String(err),
        id: server.id,
        cli: cliKey,
      });
      toast(`操作失败：${String(err)}`);
    } finally {
      setToggling(false);
    }
  }

  async function confirmDelete() {
    if (!deleteTarget) return;
    if (saving) return;
    const target = deleteTarget;
    setSaving(true);
    try {
      const ok = await mcpServerDelete(target.id);
      if (!ok) {
        toast("仅在 Tauri Desktop 环境可用");
        return;
      }
      setItems((prev) => prev.filter((s) => s.id !== target.id));
      logToConsole("info", "删除 MCP Server", { id: target.id, server_key: target.server_key });
      toast("已删除");
      setDeleteTarget(null);
    } catch (err) {
      logToConsole("error", "删除 MCP Server 失败", { error: String(err), id: target.id });
      toast(`删除失败：${String(err)}`);
    } finally {
      setSaving(false);
    }
  }

  async function parseImportJson() {
    if (importParsing) return;
    setImportParsing(true);
    setImportParseError(null);
    setImportPreview(null);
    try {
      // Best-effort: refresh current items to make the import preview accurate.
      try {
        await refresh();
      } catch {
        // ignore
      }

      const result = await mcpParseJson(importText);
      if (!result) {
        setImportPreview(null);
        setImportParseError("仅在 Tauri Desktop 环境可用");
        toast("仅在 Tauri Desktop 环境可用");
        return;
      }
      setImportPreview(result.servers ?? []);
      logToConsole("info", "解析 MCP JSON 导入", { count: result.servers?.length ?? 0 });
      toast(`解析完成：${result.servers.length} 条`);
    } catch (err) {
      const msg = formatUnknownError(err);
      setImportPreview(null);
      setImportParseError(msg);
      logToConsole("error", "解析 MCP JSON 失败", { error: msg });
      toast("解析失败：请查看弹窗内错误提示");
    } finally {
      setImportParsing(false);
    }
  }

  async function confirmImport() {
    if (importing) return;
    if (!importPreview) return;
    const effective = importAnalysis?.effective_servers ?? [];
    if (effective.length === 0) {
      toast("没有可导入的有效条目（请先修复异常或删除重复项）");
      return;
    }
    setImporting(true);
    try {
      const skippedDuplicate = importAnalysis?.summary.duplicate ?? 0;
      const skippedInvalid = importAnalysis?.summary.invalid ?? 0;

      const report = await mcpImportServers(effective);
      if (!report) {
        toast("仅在 Tauri Desktop 环境可用");
        return;
      }
      logToConsole("info", "批量导入 MCP", {
        ...report,
        insert_names: importAnalysis?.insert_names ?? [],
        update_names: importAnalysis?.update_names ?? [],
        skipped_duplicates: importAnalysis?.skipped_duplicates ?? [],
        skipped_invalid: importAnalysis?.skipped_invalid ?? [],
      });
      const skippedParts = [
        skippedDuplicate ? `覆盖跳过 ${skippedDuplicate}` : null,
        skippedInvalid ? `异常跳过 ${skippedInvalid}` : null,
      ].filter(Boolean);
      toast(
        `已导入：新增 ${report.inserted}，更新 ${report.updated}${
          skippedParts.length ? `（${skippedParts.join(" / ")}）` : ""
        }`
      );
      setImportOpen(false);
      setImportText("");
      setImportPreview(null);
      setImportParseError(null);
      await refresh();
    } catch (err) {
      logToConsole("error", "导入 MCP Servers 失败", { error: String(err) });
      toast(`导入失败：${String(err)}`);
    } finally {
      setImporting(false);
    }
  }

  return (
    <div className="space-y-3">
      <h1 className="text-2xl font-semibold tracking-tight">MCP</h1>
      <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
        <div className="flex flex-wrap items-center gap-2">
          <span className="text-xs text-slate-500">
            {loading ? "加载中…" : `共 ${items.length} 条`}
          </span>
        </div>

        <div className="flex flex-wrap items-center gap-2">
          <Button
            onClick={() => {
              setImportOpen(true);
              setImportPreview(null);
            }}
            variant="secondary"
          >
            批量导入
          </Button>
          <Button
            onClick={() => {
              setEditTarget(null);
              setDialogOpen(true);
            }}
            variant="primary"
          >
            添加 MCP
          </Button>
        </div>
      </div>

      {loading ? (
        <div className="text-sm text-slate-600">加载中…</div>
      ) : items.length === 0 ? (
        <div className="text-sm text-slate-600">
          暂无 MCP 服务。点击右上角「添加 MCP」创建第一条，或使用「导入 JSON」。
        </div>
      ) : (
        <div className="space-y-2">
          {items.map((s) => (
            <Card key={s.id} padding="md">
              <div className="flex flex-col gap-4 sm:flex-row sm:items-center sm:justify-between">
                <div className="flex items-start gap-4 min-w-0">
                  <div className="flex h-12 w-12 shrink-0 items-center justify-center rounded-xl bg-slate-100 text-slate-500 ring-1 ring-slate-200">
                    {s.transport === "http" ? (
                      <Globe className="h-6 w-6" />
                    ) : (
                      <Terminal className="h-6 w-6" />
                    )}
                  </div>

                  <div className="min-w-0 space-y-1">
                    <div className="flex items-center gap-2">
                      <div className="truncate text-base font-semibold text-slate-900 leading-tight">
                        {s.name}
                      </div>
                      <span className="inline-flex items-center gap-1 rounded-md bg-slate-100 px-1.5 py-0.5 text-[10px] font-medium text-slate-600 border border-slate-200 uppercase tracking-wider">
                        {s.transport}
                      </span>
                    </div>

                    <div className="flex items-center gap-3 text-xs text-slate-500">
                      <div
                        className="flex items-center gap-1 truncate max-w-[200px] sm:max-w-xs"
                        title={describeServer(s)}
                      >
                        {s.transport === "http" ? (
                          <Link className="h-3 w-3 shrink-0" />
                        ) : (
                          <Command className="h-3 w-3 shrink-0" />
                        )}
                        <span className="truncate">{describeServer(s)}</span>
                      </div>
                    </div>
                  </div>
                </div>

                <div className="flex items-center justify-between gap-4 sm:justify-end">
                  <div className="flex items-center gap-2">
                    {CLIS.map((cli) => {
                      const checked =
                        cli.key === "claude"
                          ? s.enabled_claude
                          : cli.key === "codex"
                            ? s.enabled_codex
                            : s.enabled_gemini;
                      return (
                        <div
                          key={cli.key}
                          className="flex flex-col items-center gap-1"
                          title={`Toggle for ${cli.name}`}
                        >
                          <Switch
                            checked={checked}
                            disabled={toggling}
                            onCheckedChange={() => void toggleEnabled(s, cli.key)}
                            className="scale-90"
                          />
                          <span className="text-[10px] font-medium text-slate-400">{cli.name}</span>
                        </div>
                      );
                    })}
                  </div>

                  <div className="h-8 w-px bg-slate-200" />

                  <div className="flex items-center gap-1">
                    <Button
                      onClick={() => {
                        setEditTarget(s);
                        setDialogOpen(true);
                      }}
                      size="sm"
                      variant="ghost"
                      className="h-8 w-8 p-0 text-slate-500 hover:text-indigo-600 hover:bg-indigo-50"
                      title="编辑"
                    >
                      <Edit2 className="h-4 w-4" />
                    </Button>
                    <Button
                      onClick={() => setDeleteTarget(s)}
                      size="sm"
                      variant="ghost"
                      className="h-8 w-8 p-0 text-slate-400 hover:text-rose-600 hover:bg-rose-50"
                      title="删除"
                    >
                      <Trash2 className="h-4 w-4" />
                    </Button>
                  </div>
                </div>
              </div>
            </Card>
          ))}
        </div>
      )}

      <Dialog
        open={dialogOpen}
        title={editTarget ? "编辑 MCP 服务" : "添加 MCP 服务"}
        description={
          editTarget ? "修改后会自动同步到启用的 CLI 配置文件。" : `类型：${transportHint}`
        }
        onOpenChange={(open) => {
          setDialogOpen(open);
          if (!open) setFillDialogOpen(false);
          if (!open) setEditTarget(null);
        }}
        className="max-w-3xl"
      >
        <div className="grid gap-4">
          <div className="rounded-2xl border border-slate-200 bg-gradient-to-b from-white to-slate-50/60 p-4 shadow-card">
            <div className="flex flex-wrap items-center justify-between gap-2">
              <div className="text-xs font-medium text-slate-500">基础信息</div>
              <Button size="sm" variant="secondary" onClick={() => setFillDialogOpen(true)}>
                从 JSON 回填
              </Button>
            </div>

            <div className="mt-3">
              <div className="text-sm font-medium text-slate-700">名称</div>
              <input
                type="text"
                value={name}
                onChange={(e) => setName(e.currentTarget.value)}
                placeholder="例如：Fetch 工具"
                className="mt-2 w-full rounded-xl border border-slate-200 bg-white px-3 py-2 text-sm text-slate-900 shadow-sm outline-none focus:border-[#0052FF] focus:ring-2 focus:ring-[#0052FF]/20"
              />
            </div>

            <div className="mt-4">
              <div className="flex items-center justify-between gap-3">
                <div className="text-sm font-medium text-slate-700">类型</div>
                <div className="text-xs text-slate-500">二选一</div>
              </div>
              <div className="mt-2 grid gap-2 sm:grid-cols-2">
                {(
                  [
                    {
                      value: "stdio",
                      title: "STDIO",
                      desc: "本地命令（通过 command/args 启动）",
                      icon: "⌘",
                    },
                    {
                      value: "http",
                      title: "HTTP",
                      desc: "远程服务（通过 URL 调用）",
                      icon: "⇄",
                    },
                  ] as const
                ).map((item) => (
                  <label key={item.value} className="relative block">
                    <input
                      type="radio"
                      name="mcp-transport"
                      value={item.value}
                      checked={transport === item.value}
                      onChange={() => setTransport(item.value)}
                      className="peer sr-only"
                    />
                    <div
                      className={cn(
                        "flex h-full cursor-pointer items-start gap-3 rounded-xl border px-3 py-3 shadow-sm transition-all",
                        "bg-white",
                        "hover:border-slate-300 hover:bg-slate-50/60 hover:shadow",
                        "peer-focus-visible:ring-2 peer-focus-visible:ring-[#0052FF]/20 peer-focus-visible:ring-offset-2 peer-focus-visible:ring-offset-white",
                        "peer-checked:border-[#0052FF]/60 peer-checked:bg-[#0052FF]/5 peer-checked:shadow"
                      )}
                    >
                      <div
                        className={cn(
                          "mt-0.5 flex h-9 w-9 items-center justify-center rounded-lg border bg-white shadow-sm",
                          "border-slate-200 text-slate-700",
                          "peer-checked:border-[#0052FF]/40 peer-checked:bg-[#0052FF]/10 peer-checked:text-[#0052FF]"
                        )}
                      >
                        <span className="text-sm font-semibold">{item.icon}</span>
                      </div>

                      <div className="min-w-0 pr-7">
                        <div className="text-sm font-semibold text-slate-900">{item.title}</div>
                        <div className="mt-0.5 text-xs leading-relaxed text-slate-500">
                          {item.desc}
                        </div>
                      </div>
                    </div>

                    <div className="pointer-events-none absolute right-3 top-3 flex h-5 w-5 items-center justify-center rounded-full border border-slate-300 bg-white text-[11px] text-white shadow-sm transition peer-checked:border-[#0052FF] peer-checked:bg-[#0052FF]">
                      ✓
                    </div>
                  </label>
                ))}
              </div>
            </div>
          </div>

          <div>
            <div className="text-sm font-medium text-slate-700">生效范围</div>
            <div className="mt-2 flex flex-wrap items-center gap-3">
              <div className="flex items-center gap-2">
                <Switch checked={enabledClaude} onCheckedChange={setEnabledClaude} />
                <span className="text-sm text-slate-700">Claude</span>
              </div>
              <div className="flex items-center gap-2">
                <Switch checked={enabledCodex} onCheckedChange={setEnabledCodex} />
                <span className="text-sm text-slate-700">Codex</span>
              </div>
              <div className="flex items-center gap-2">
                <Switch checked={enabledGemini} onCheckedChange={setEnabledGemini} />
                <span className="text-sm text-slate-700">Gemini</span>
              </div>
            </div>
          </div>

          {transport === "stdio" ? (
            <>
              <div>
                <div className="text-sm font-medium text-slate-700">Command</div>
                <input
                  type="text"
                  value={command}
                  onChange={(e) => setCommand(e.currentTarget.value)}
                  placeholder="例如：npx"
                  className="mt-2 w-full rounded-lg border border-slate-200 bg-white px-3 py-2 font-mono text-sm text-slate-900 shadow-sm outline-none focus:border-[#0052FF] focus:ring-2 focus:ring-[#0052FF]/20"
                />
              </div>

              <div className="grid gap-3 sm:grid-cols-2">
                <div>
                  <div className="text-sm font-medium text-slate-700">Args（每行一个）</div>
                  <textarea
                    value={argsText}
                    onChange={(e) => setArgsText(e.currentTarget.value)}
                    placeholder={`例如：\n-y\n@modelcontextprotocol/server-fetch`}
                    rows={6}
                    className="mt-2 w-full resize-y rounded-lg border border-slate-200 bg-white px-3 py-2 font-mono text-xs text-slate-900 shadow-sm outline-none focus:border-[#0052FF] focus:ring-2 focus:ring-[#0052FF]/20"
                  />
                </div>

                <div>
                  <div className="text-sm font-medium text-slate-700">Env（每行 KEY=VALUE）</div>
                  <textarea
                    value={envText}
                    onChange={(e) => setEnvText(e.currentTarget.value)}
                    placeholder={`例如：\nFOO=bar\nTOKEN=xxx`}
                    rows={6}
                    className="mt-2 w-full resize-y rounded-lg border border-slate-200 bg-white px-3 py-2 font-mono text-xs text-slate-900 shadow-sm outline-none focus:border-[#0052FF] focus:ring-2 focus:ring-[#0052FF]/20"
                  />
                </div>
              </div>

              <div>
                <div className="text-sm font-medium text-slate-700">CWD（可选）</div>
                <input
                  type="text"
                  value={cwd}
                  onChange={(e) => setCwd(e.currentTarget.value)}
                  placeholder="例如：/Users/xxx/project"
                  className="mt-2 w-full rounded-lg border border-slate-200 bg-white px-3 py-2 font-mono text-sm text-slate-900 shadow-sm outline-none focus:border-[#0052FF] focus:ring-2 focus:ring-[#0052FF]/20"
                />
              </div>
            </>
          ) : (
            <>
              <div>
                <div className="text-sm font-medium text-slate-700">URL</div>
                <input
                  type="text"
                  value={url}
                  onChange={(e) => setUrl(e.currentTarget.value)}
                  placeholder="例如：https://example.com/mcp"
                  className="mt-2 w-full rounded-lg border border-slate-200 bg-white px-3 py-2 font-mono text-sm text-slate-900 shadow-sm outline-none focus:border-[#0052FF] focus:ring-2 focus:ring-[#0052FF]/20"
                />
              </div>

              <div>
                <div className="text-sm font-medium text-slate-700">Headers（每行 KEY=VALUE）</div>
                <textarea
                  value={headersText}
                  onChange={(e) => setHeadersText(e.currentTarget.value)}
                  placeholder={`例如：\nAuthorization=Bearer xxx\nX-Env=dev`}
                  rows={6}
                  className="mt-2 w-full resize-y rounded-lg border border-slate-200 bg-white px-3 py-2 font-mono text-xs text-slate-900 shadow-sm outline-none focus:border-[#0052FF] focus:ring-2 focus:ring-[#0052FF]/20"
                />
              </div>
            </>
          )}

          <div className="flex flex-wrap items-center gap-2">
            <Button
              onClick={save}
              variant="primary"
              disabled={saving || (transport === "stdio" ? !command.trim() : !url.trim())}
            >
              {saving ? "保存中…" : "保存并同步"}
            </Button>
            <Button
              onClick={() => {
                setDialogOpen(false);
                setEditTarget(null);
              }}
              variant="secondary"
              disabled={saving}
            >
              取消
            </Button>
          </div>
        </div>
      </Dialog>

      <Dialog
        open={fillDialogOpen}
        title="从 JSON 回填"
        description="粘贴 JSON → 解析预览 → 回填到当前表单（会覆盖当前输入）。"
        onOpenChange={(open) => setFillDialogOpen(open)}
        className="max-w-4xl"
      >
        <div className="grid gap-3">
          <div>
            <div className="text-sm font-medium text-slate-700">JSON</div>
            <textarea
              value={fillText}
              onChange={(e) => setFillText(e.currentTarget.value)}
              placeholder='示例：[{"name":"Fetch","transport":"stdio","command":"npx","args":["-y","@modelcontextprotocol/server-fetch"]}]'
              rows={10}
              className="mt-2 w-full resize-y rounded-xl border border-slate-200 bg-white px-3 py-2 font-mono text-xs text-slate-900 shadow-sm outline-none focus:border-[#0052FF] focus:ring-2 focus:ring-[#0052FF]/20"
            />
          </div>

          <div className="flex flex-wrap items-center gap-2">
            <Button
              onClick={() => void parseFillOptions()}
              variant="secondary"
              disabled={fillParsing || !fillText.trim()}
            >
              {fillParsing ? "解析中…" : "解析预览"}
            </Button>
            <Button
              onClick={() => void importAndFill()}
              variant="primary"
              disabled={
                fillParsing || (!fillText.trim() && (!fillOptions || fillOptions.length === 0))
              }
            >
              导入并回填
            </Button>
            <Button
              onClick={() => {
                setFillText("");
                setFillOptions(null);
                setFillIndex(0);
              }}
              variant="secondary"
              disabled={fillParsing}
            >
              清空
            </Button>

            <span className="text-xs text-slate-500">
              {fillOptions ? `已解析：${fillOptions.length} 条` : "未解析"}
            </span>
          </div>

          {fillOptions && fillOptions.length > 1 ? (
            <div className="rounded-xl border border-slate-200 bg-white p-3">
              <div className="flex flex-wrap items-center gap-2">
                <div className="text-sm font-medium text-slate-700">选择回填项</div>
                <select
                  value={String(fillIndex)}
                  onChange={(e) => setFillIndex(Number(e.currentTarget.value))}
                  className="h-9 rounded-xl border border-slate-200 bg-white px-2 text-sm text-slate-900 shadow-sm outline-none focus:border-[#0052FF] focus:ring-2 focus:ring-[#0052FF]/20"
                >
                  {fillOptions.map((s, idx) => (
                    <option key={`${s.server_key}-${s.name}`} value={String(idx)}>
                      {s.name}
                    </option>
                  ))}
                </select>
                <span className="text-xs text-slate-500">共 {fillOptions.length} 条</span>
              </div>

              {selectedFillServer() ? (
                <div className="mt-2 text-xs text-slate-500">
                  预览：{selectedFillServer()!.transport.toUpperCase()} ·{" "}
                  {selectedFillServer()!.transport === "http"
                    ? selectedFillServer()!.url
                    : selectedFillServer()!.command}
                </div>
              ) : null}
            </div>
          ) : null}
        </div>
      </Dialog>

      <Dialog
        open={Boolean(deleteTarget)}
        title="确认删除"
        description={
          deleteTarget
            ? `将删除「${deleteTarget.name}」并从已启用的 CLI 配置中移除（不可恢复）。`
            : undefined
        }
        onOpenChange={(open) => {
          if (!open) setDeleteTarget(null);
        }}
        className="max-w-xl"
      >
        <div className="flex flex-wrap items-center gap-2">
          <Button onClick={confirmDelete} variant="primary" disabled={saving}>
            {saving ? "删除中…" : "确认删除"}
          </Button>
          <Button onClick={() => setDeleteTarget(null)} variant="secondary" disabled={saving}>
            取消
          </Button>
        </div>
      </Dialog>

      <Dialog
        open={importOpen}
        title="导入 MCP JSON"
        description="粘贴 JSON → 解析预览 → 确认导入（兼容 code-switch-R 的 mcp.json，或数组格式）。"
        onOpenChange={(open) => {
          setImportOpen(open);
          if (!open) {
            setImportText("");
            setImportPreview(null);
            setImportParseError(null);
          }
        }}
        className="max-w-4xl"
      >
        <div className="grid gap-4">
          <div>
            <div className="text-sm font-medium text-slate-700">JSON</div>
            <textarea
              value={importText}
              onChange={(e) => {
                setImportText(e.currentTarget.value);
                setImportPreview(null);
                setImportParseError(null);
              }}
              placeholder='粘贴 JSON（数组示例：[{"name":"Fetch","transport":"stdio","command":"npx","args":["-y","@modelcontextprotocol/server-fetch"]}]）'
              rows={10}
              className="mt-2 w-full resize-y rounded-lg border border-slate-200 bg-white px-3 py-2 font-mono text-xs text-slate-900 shadow-sm outline-none focus:border-[#0052FF] focus:ring-2 focus:ring-[#0052FF]/20"
            />
          </div>

          <div className="flex flex-wrap items-center gap-2">
            <Button
              onClick={() => void parseImportJson()}
              variant="secondary"
              disabled={importParsing || !importText.trim()}
            >
              {importParsing ? "解析中…" : "解析预览"}
            </Button>
            <Button
              onClick={() => void confirmImport()}
              variant="primary"
              disabled={importing || !importAnalysis || importAnalysis.summary.effective === 0}
            >
              {importing
                ? "导入中…"
                : `确认导入（有效 ${importAnalysis?.summary.effective ?? 0} 条）`}
            </Button>
            <span className="text-xs text-slate-500">
              {importAnalysis
                ? `解析：${importAnalysis.summary.total} 条（新增 ${importAnalysis.summary.insert} / 更新 ${importAnalysis.summary.update} / 覆盖 ${importAnalysis.summary.duplicate} / 异常 ${importAnalysis.summary.invalid}）`
                : "未解析"}
            </span>
          </div>

          {importParseErrorView ? (
            <div className="rounded-xl border border-rose-200 bg-rose-50 p-3">
              <div className="grid gap-2">
                <div className="text-sm font-semibold text-rose-900">
                  {importParseErrorView.title}
                </div>
                <div className="text-xs text-rose-700">{importParseErrorView.summary}</div>
                {importParseErrorView.details.length ? (
                  <div className="grid gap-1">
                    {importParseErrorView.details.map((d) => (
                      <div
                        key={d.label}
                        className="flex flex-wrap items-baseline gap-x-1 text-xs text-rose-800"
                      >
                        <span className="font-medium">{d.label}：</span>
                        <span className={cn(d.mono ? "font-mono" : undefined)}>{d.value}</span>
                      </div>
                    ))}
                  </div>
                ) : null}
                {importParseErrorView.hint ? (
                  <div className="text-xs text-rose-700">建议：{importParseErrorView.hint}</div>
                ) : null}
                <div className="text-[11px] text-rose-700/80">
                  原始错误：{importParseErrorView.raw}
                </div>
              </div>
            </div>
          ) : null}

          {importAnalysis ? (
            importAnalysis.summary.total === 0 ? (
              <div className="text-sm text-slate-600">解析成功，但没有找到可导入的 MCP 配置。</div>
            ) : (
              <div className="grid gap-4">
                <div className="rounded-xl border border-slate-200 bg-white p-3 text-xs text-slate-600">
                  规则：按名称匹配（trim +
                  不区分大小写）；同名后者覆盖前者（last-wins）；更新同名会覆盖全部字段（含生效范围）。
                </div>

                <div className="grid gap-2 sm:grid-cols-4">
                  <div className="rounded-xl border border-emerald-200 bg-emerald-50 p-3">
                    <div className="text-xs font-medium text-emerald-700">将新增</div>
                    <div className="mt-1 text-2xl font-semibold text-emerald-900">
                      {importAnalysis.summary.insert}
                    </div>
                  </div>
                  <div className="rounded-xl border border-blue-200 bg-blue-50 p-3">
                    <div className="text-xs font-medium text-blue-700">将更新</div>
                    <div className="mt-1 text-2xl font-semibold text-blue-900">
                      {importAnalysis.summary.update}
                    </div>
                  </div>
                  <div className="rounded-xl border border-amber-200 bg-amber-50 p-3">
                    <div className="text-xs font-medium text-amber-700">被覆盖</div>
                    <div className="mt-1 text-2xl font-semibold text-amber-900">
                      {importAnalysis.summary.duplicate}
                    </div>
                  </div>
                  <div className="rounded-xl border border-rose-200 bg-rose-50 p-3">
                    <div className="text-xs font-medium text-rose-700">异常</div>
                    <div className="mt-1 text-2xl font-semibold text-rose-900">
                      {importAnalysis.summary.invalid}
                    </div>
                  </div>
                </div>

                {(
                  [
                    {
                      status: "insert" as const,
                      title: "将新增",
                      badge: "bg-emerald-50 text-emerald-700",
                    },
                    {
                      status: "update" as const,
                      title: "将更新",
                      badge: "bg-blue-50 text-blue-700",
                    },
                    {
                      status: "duplicate" as const,
                      title: "被覆盖（不导入）",
                      badge: "bg-amber-50 text-amber-700",
                    },
                    {
                      status: "invalid" as const,
                      title: "异常（不导入）",
                      badge: "bg-rose-50 text-rose-700",
                    },
                  ] as const
                ).map((group) => {
                  const list = importAnalysis.rows.filter((r) => r.status === group.status);
                  if (list.length === 0) return null;
                  const shown = list.slice(0, 20);

                  return (
                    <div key={group.status} className="space-y-2">
                      <div className="flex items-center justify-between">
                        <div className="text-sm font-semibold text-slate-900">{group.title}</div>
                        <span className="text-xs text-slate-500">{list.length} 条</span>
                      </div>

                      <div className="space-y-2">
                        {shown.map((row, idx) => (
                          <Card
                            key={`${row.server.server_key}-${row.server.name}-${idx}`}
                            padding="sm"
                          >
                            <div className="flex flex-col gap-2 sm:flex-row sm:items-start sm:justify-between">
                              <div className="min-w-0">
                                <div className="flex flex-wrap items-center gap-2">
                                  <div className="truncate text-sm font-semibold text-slate-900">
                                    {row.server.name || "（未命名）"}
                                  </div>
                                  <span
                                    className={cn(
                                      "rounded-full px-2 py-0.5 text-xs font-medium",
                                      group.badge
                                    )}
                                  >
                                    {group.status === "insert"
                                      ? "新增"
                                      : group.status === "update"
                                        ? "更新"
                                        : group.status === "duplicate"
                                          ? "覆盖"
                                          : "异常"}
                                  </span>
                                  <span className="rounded-full bg-slate-100 px-2 py-0.5 text-xs font-medium text-slate-700">
                                    {row.server.transport.toUpperCase()}
                                  </span>
                                </div>
                                <div className="mt-1 text-xs text-slate-500">
                                  {row.server.transport === "http"
                                    ? row.server.url
                                    : row.server.command}
                                </div>
                                {row.reason ? (
                                  <div className="mt-1 text-xs text-rose-700">{row.reason}</div>
                                ) : null}
                              </div>

                              <div className="text-xs text-slate-500">
                                {enabledLabelFromImport(row.server)}
                              </div>
                            </div>
                          </Card>
                        ))}
                        {list.length > shown.length ? (
                          <div className="text-xs text-slate-500">
                            仅展示前 {shown.length} 条，实际 {list.length} 条。
                          </div>
                        ) : null}
                      </div>
                    </div>
                  );
                })}
              </div>
            )
          ) : null}
        </div>
      </Dialog>
    </div>
  );
}
