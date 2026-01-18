import { useSyncExternalStore } from "react";
import { gatewayStatus, type GatewayStatus } from "../services/gateway";
import { settingsGet } from "../services/settings";
import { hasTauriRuntime } from "../services/tauriInvoke";

export type GatewayAvailability = "checking" | "available" | "unavailable";

export type GatewayMeta = {
  gatewayAvailable: GatewayAvailability;
  gateway: GatewayStatus | null;
  preferredPort: number;
};

type Listener = () => void;

let snapshot: GatewayMeta = {
  gatewayAvailable: "checking",
  gateway: null,
  preferredPort: 37123,
};

const listeners = new Set<Listener>();

let started = false;
let starting: Promise<void> | null = null;
let unlistenStatus: (() => void) | null = null;

function emit() {
  for (const listener of listeners) listener();
}

function setSnapshot(patch: Partial<GatewayMeta>) {
  snapshot = { ...snapshot, ...patch };
  emit();
}

export function gatewayMetaSetPreferredPort(port: number) {
  const next = Number.isFinite(port) ? Math.floor(port) : snapshot.preferredPort;
  if (next <= 0 || next > 65535) return;
  if (snapshot.preferredPort === next) return;
  setSnapshot({ preferredPort: next });
}

function gatewayMetaSetGateway(status: GatewayStatus) {
  setSnapshot({ gatewayAvailable: "available", gateway: status });
}

async function ensureStarted() {
  if (started) return;
  if (starting) return starting;

  starting = (async () => {
    if (!hasTauriRuntime()) {
      setSnapshot({ gatewayAvailable: "unavailable", gateway: null });
      started = true;
      starting = null;
      return;
    }

    setSnapshot({ gatewayAvailable: "checking" });

    try {
      const [gatewayRes, settingsRes] = await Promise.allSettled([gatewayStatus(), settingsGet()]);

      if (settingsRes.status === "fulfilled" && settingsRes.value) {
        gatewayMetaSetPreferredPort(settingsRes.value.preferred_port);
      }

      if (gatewayRes.status === "fulfilled" && gatewayRes.value) {
        gatewayMetaSetGateway(gatewayRes.value);
      } else {
        setSnapshot({ gatewayAvailable: "unavailable", gateway: null });
      }
    } catch {
      setSnapshot({ gatewayAvailable: "unavailable", gateway: null });
    }

    if (!unlistenStatus) {
      try {
        const { listen } = await import("@tauri-apps/api/event");
        unlistenStatus = await listen<GatewayStatus>("gateway:status", (event) => {
          if (!event.payload) return;
          gatewayMetaSetGateway(event.payload);
        });
      } catch {
        // ignore: events unavailable in non-tauri environment
      }
    }

    started = true;
    starting = null;
  })();

  return starting;
}

export function useGatewayMeta(): GatewayMeta {
  return useSyncExternalStore(
    (listener) => {
      listeners.add(listener);
      void ensureStarted();
      return () => listeners.delete(listener);
    },
    () => snapshot,
    () => snapshot
  );
}
