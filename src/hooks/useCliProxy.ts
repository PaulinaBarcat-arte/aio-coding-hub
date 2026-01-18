import { useCallback, useEffect, useRef, useState } from "react";
import { toast } from "sonner";
import { cliProxySetEnabled, cliProxyStatusAll } from "../services/cliProxy";
import { logToConsole } from "../services/consoleLog";
import type { CliKey } from "../services/providers";

const DEFAULT_ENABLED: Record<CliKey, boolean> = {
  claude: true,
  codex: false,
  gemini: false,
};

const DEFAULT_TOGGLING: Record<CliKey, boolean> = {
  claude: false,
  codex: false,
  gemini: false,
};

export function useCliProxy() {
  const [enabled, setEnabled] = useState<Record<CliKey, boolean>>(DEFAULT_ENABLED);
  const [toggling, setToggling] = useState<Record<CliKey, boolean>>(DEFAULT_TOGGLING);

  const enabledRef = useRef(enabled);
  const togglingRef = useRef(toggling);

  useEffect(() => {
    enabledRef.current = enabled;
  }, [enabled]);

  useEffect(() => {
    togglingRef.current = toggling;
  }, [toggling]);

  const refresh = useCallback(() => {
    let cancelled = false;
    cliProxyStatusAll()
      .then((statuses) => {
        if (cancelled) return;
        if (!statuses) return;
        setEnabled((prev) => {
          const next = { ...prev };
          for (const row of statuses) {
            next[row.cli_key] = Boolean(row.enabled);
          }
          return next;
        });
      })
      .catch((err) => {
        if (cancelled) return;
        logToConsole("warn", "读取 CLI 代理状态失败", { error: String(err) });
      });

    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    const cleanup = refresh();
    return cleanup;
  }, [refresh]);

  const setCliProxyEnabled = useCallback((cliKey: CliKey, next: boolean) => {
    if (togglingRef.current[cliKey]) return;

    const prev = enabledRef.current[cliKey];
    setEnabled((cur) => ({ ...cur, [cliKey]: next }));
    setToggling((cur) => ({ ...cur, [cliKey]: true }));

    cliProxySetEnabled({ cli_key: cliKey, enabled: next })
      .then((res) => {
        if (!res) {
          toast("仅在 Tauri Desktop 环境可用");
          setEnabled((cur) => ({ ...cur, [cliKey]: prev }));
          return;
        }

        if (res.ok) {
          toast(res.message || (next ? "已开启代理" : "已关闭代理"));
          logToConsole("info", next ? "开启 CLI 代理" : "关闭 CLI 代理", res);
          setEnabled((cur) => ({ ...cur, [cliKey]: Boolean(res.enabled) }));
        } else {
          toast(res.message ? `操作失败：${res.message}` : "操作失败");
          logToConsole("error", next ? "开启 CLI 代理失败" : "关闭 CLI 代理失败", res);
          setEnabled((cur) => ({ ...cur, [cliKey]: prev }));
        }
      })
      .catch((err) => {
        toast(`操作失败：${String(err)}`);
        logToConsole("error", "切换 CLI 代理失败", {
          cli: cliKey,
          enabled: next,
          error: String(err),
        });
        setEnabled((cur) => ({ ...cur, [cliKey]: prev }));
      })
      .finally(() => {
        setToggling((cur) => ({ ...cur, [cliKey]: false }));
      });
  }, []);

  return {
    enabled,
    toggling,
    refresh,
    setCliProxyEnabled,
  };
}
