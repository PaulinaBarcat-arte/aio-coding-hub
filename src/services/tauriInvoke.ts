export function hasTauriRuntime() {
  return typeof window !== "undefined" && typeof (window as any).__TAURI_INTERNALS__ === "object";
}

export async function invokeTauriOrNull<T>(
  cmd: string,
  args?: Record<string, unknown>
): Promise<T | null> {
  if (!hasTauriRuntime()) return null;
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<T>(cmd, args);
}
