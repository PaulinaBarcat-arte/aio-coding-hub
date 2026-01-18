// Usage: Shared types for `src/pages/providers/*` modules.

export type ProviderBaseUrlMode = "order" | "ping";

export type BaseUrlPingState =
  | { status: "idle" }
  | { status: "pinging" }
  | { status: "ok"; ms: number }
  | { status: "error"; message: string };

export type BaseUrlRow = {
  id: string;
  url: string;
  ping: BaseUrlPingState;
};
