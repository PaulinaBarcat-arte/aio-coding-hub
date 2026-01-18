// Usage: Client-side validators for provider-related forms (toast-based UX).

export function validateProviderName(name: string) {
  if (name.trim()) return null;
  return "名称不能为空";
}

export function validateProviderApiKeyForCreate(apiKey: string) {
  if (apiKey.trim()) return null;
  return "API Key 不能为空（新增 Provider 必填）";
}

export function parseAndValidateCostMultiplier(raw: string) {
  const value = Number(raw);
  if (!Number.isFinite(value)) {
    return { ok: false as const, message: "价格倍率必须是数字" };
  }
  if (value <= 0) {
    return { ok: false as const, message: "价格倍率必须大于 0" };
  }
  if (value > 1000) {
    return { ok: false as const, message: "价格倍率不能大于 1000" };
  }
  return { ok: true as const, value };
}
