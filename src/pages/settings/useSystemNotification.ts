import { useEffect, useState } from "react";
import { toast } from "sonner";
import { logToConsole } from "../../services/consoleLog";
import { noticeSend } from "../../services/notice";

export type NoticePermissionStatus = "checking" | "granted" | "not_granted" | "denied" | "unknown";

type NotificationPluginModule = typeof import("@tauri-apps/plugin-notification");

let notificationPluginPromise: Promise<NotificationPluginModule> | null = null;

function loadNotificationPlugin(): Promise<NotificationPluginModule> {
  if (notificationPluginPromise) return notificationPluginPromise;
  notificationPluginPromise = import("@tauri-apps/plugin-notification").catch((err) => {
    notificationPluginPromise = null;
    throw err;
  });
  return notificationPluginPromise;
}

export function useSystemNotification() {
  const [noticePermissionStatus, setNoticePermissionStatus] =
    useState<NoticePermissionStatus>("checking");
  const [requestingNoticePermission, setRequestingNoticePermission] = useState(false);
  const [sendingNoticeTest, setSendingNoticeTest] = useState(false);

  useEffect(() => {
    let cancelled = false;
    loadNotificationPlugin()
      .then(async ({ isPermissionGranted }) => {
        const granted = await isPermissionGranted();
        if (cancelled) return;
        setNoticePermissionStatus(granted ? "granted" : "not_granted");
      })
      .catch((err) => {
        if (cancelled) return;
        logToConsole("error", "检查系统通知权限失败", { error: String(err) });
        setNoticePermissionStatus("unknown");
      });

    return () => {
      cancelled = true;
    };
  }, []);

  async function requestSystemNotificationPermission() {
    if (requestingNoticePermission) return;
    setRequestingNoticePermission(true);

    try {
      const { requestPermission } = await loadNotificationPlugin();
      const permission = await requestPermission();
      const granted = permission === "granted";
      setNoticePermissionStatus(granted ? "granted" : "denied");
      toast(granted ? "系统通知权限已授权" : "系统通知权限已拒绝");
    } catch (err) {
      logToConsole("error", "请求系统通知权限失败", { error: String(err) });
      toast("请求系统通知权限失败：请查看控制台日志");
      setNoticePermissionStatus("unknown");
    } finally {
      setRequestingNoticePermission(false);
    }
  }

  async function sendSystemNotificationTest() {
    if (sendingNoticeTest) return;
    setSendingNoticeTest(true);

    try {
      const { isPermissionGranted } = await loadNotificationPlugin();
      const granted = await isPermissionGranted();
      if (!granted) {
        setNoticePermissionStatus("not_granted");
        toast("请先在「系统通知」中授权通知权限");
        return;
      }

      const ok = await noticeSend({
        level: "info",
        title: "测试通知",
        body: "这是一条来自 AIO Coding Hub 的系统通知",
      });
      if (!ok) {
        toast("仅在 Tauri Desktop 环境可用");
        return;
      }

      toast("已发送测试通知");
    } catch (err) {
      logToConsole("error", "发送测试通知失败", { error: String(err) });
      toast("发送测试通知失败：请查看控制台日志");
    } finally {
      setSendingNoticeTest(false);
    }
  }

  return {
    noticePermissionStatus,
    requestingNoticePermission,
    sendingNoticeTest,
    requestSystemNotificationPermission,
    sendSystemNotificationTest,
  };
}
