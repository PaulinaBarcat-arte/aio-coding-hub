// Usage: Runtime log console. Shows in-memory app logs (time / level / title) with optional on-demand details.
// Request log details are persisted separately and should not be displayed here.

import { memo, useEffect, useRef, useState } from "react";
import {
  clearConsoleLogs,
  formatConsoleLogDetails,
  type ConsoleLogEntry,
  useConsoleLogs,
} from "../services/consoleLog";
import { ChevronRight } from "lucide-react";
import { toast } from "sonner";
import { Button } from "../ui/Button";
import { Card } from "../ui/Card";
import { PageHeader } from "../ui/PageHeader";
import { Switch } from "../ui/Switch";
import { cn } from "../utils/cn";

function levelText(level: ConsoleLogEntry["level"]) {
  switch (level) {
    case "error":
      return "ERROR";
    case "warn":
      return "WARN";
    default:
      return "INFO";
  }
}

function levelTone(level: ConsoleLogEntry["level"]) {
  switch (level) {
    case "error":
      return "text-rose-300";
    case "warn":
      return "text-amber-300";
    default:
      return "text-emerald-300";
  }
}

const ROW_GRID_CLASS = "grid grid-cols-[180px_72px_1fr_20px] gap-3";

const ConsoleLogRow = memo(function ConsoleLogRow({ entry }: { entry: ConsoleLogEntry }) {
  const hasDetails = entry.details !== undefined;
  const [detailsText, setDetailsText] = useState<string | null>(null);

  const row = (
    <div className={cn(ROW_GRID_CLASS, "items-start px-4 py-1.5")}>
      <span className="shrink-0 text-slate-500">{entry.tsText}</span>
      <span className={cn("shrink-0 font-semibold", levelTone(entry.level))}>
        {levelText(entry.level)}
      </span>
      <span className="min-w-0 whitespace-pre-wrap break-words text-slate-100">{entry.title}</span>
      <span className="flex justify-end pt-0.5 text-slate-600 group-open:text-slate-400">
        {hasDetails ? (
          <ChevronRight className="h-4 w-4 transition-transform group-open:rotate-90" />
        ) : null}
      </span>
    </div>
  );

  if (!hasDetails) {
    return <div className="border-b border-white/5">{row}</div>;
  }

  return (
    <details
      className="group border-b border-white/5"
      onToggle={(e) => {
        if (!e.currentTarget.open) return;
        if (detailsText != null) return;
        const next = formatConsoleLogDetails(entry.details);
        setDetailsText(next ?? "");
      }}
    >
      <summary
        className={cn(
          "block cursor-pointer select-none outline-none hover:bg-white/5",
          "list-none [&::-webkit-details-marker]:hidden [&::marker]:content-none"
        )}
      >
        {row}
      </summary>
      <div className={cn(ROW_GRID_CLASS, "px-4 pb-3 pt-2")}>
        <div className="col-start-3 col-span-2">
          <pre className="custom-scrollbar max-h-60 overflow-auto rounded-lg bg-black/30 p-3 text-[11px] leading-relaxed text-slate-200">
            {detailsText == null ? "加载中…" : detailsText ? detailsText : "// 无可显示的详情"}
          </pre>
        </div>
      </div>
    </details>
  );
});

export function ConsolePage() {
  const logs = useConsoleLogs();
  const [autoScroll, setAutoScroll] = useState(true);
  const logsContainerRef = useRef<HTMLDivElement | null>(null);

  function scrollToBottom() {
    const el = logsContainerRef.current;
    if (!el) return;
    el.scrollTop = el.scrollHeight;
  }

  useEffect(() => {
    if (!autoScroll) return;
    requestAnimationFrame(() => scrollToBottom());
  }, [autoScroll, logs.length]);

  return (
    <div className="space-y-6">
      <PageHeader
        title="控制台"
        actions={
          <div className="flex flex-wrap items-center gap-3">
            <div className="flex items-center gap-2">
              <span className="text-sm text-slate-600">自动滚动</span>
              <Switch checked={autoScroll} onCheckedChange={setAutoScroll} size="sm" />
            </div>
            <Button
              onClick={() => {
                clearConsoleLogs();
                toast("已清空控制台日志");
              }}
              variant="secondary"
            >
              清空日志
            </Button>
          </div>
        }
      />

      <Card padding="none">
        <div className="border-b border-slate-200 px-4 py-3">
          <div className="flex flex-wrap items-end justify-between gap-2">
            <div className="text-sm font-medium">日志（{logs.length}）</div>
            <div className="text-xs text-slate-500">点击单条日志可展开详情</div>
          </div>
        </div>

        <div
          ref={logsContainerRef}
          className={cn(
            "custom-scrollbar max-h-[70vh] overflow-auto",
            "bg-slate-950 font-mono text-[12px] leading-relaxed text-slate-200"
          )}
        >
          {logs.length === 0 ? (
            <div className="px-4 py-10 text-sm text-slate-400">暂无日志</div>
          ) : (
            <div>
              {logs.map((entry) => (
                <ConsoleLogRow key={entry.id} entry={entry} />
              ))}
            </div>
          )}
        </div>
      </Card>
    </div>
  );
}
