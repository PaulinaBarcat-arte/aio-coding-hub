// Usage:
// - Used by `src/components/home/HomeCostPanel.tsx` to load cost analytics for the Home "花费" tab.

import { invokeTauriOrNull } from "./tauriInvoke";
import type { CliKey } from "./providers";

export type CostPeriod = "daily" | "weekly" | "monthly" | "allTime" | "custom";

export type CostSummaryV1 = {
  requests_total: number;
  requests_success: number;
  requests_failed: number;
  cost_covered_success: number;
  total_cost_usd: number;
  avg_cost_usd_per_covered_success: number | null;
};

export type CostTrendRowV1 = {
  day: string;
  hour: number | null;
  cost_usd: number;
  requests_success: number;
  cost_covered_success: number;
};

export type CostProviderBreakdownRowV1 = {
  cli_key: CliKey;
  provider_id: number;
  provider_name: string;
  requests_success: number;
  cost_covered_success: number;
  cost_usd: number;
};

export type CostModelBreakdownRowV1 = {
  model: string;
  requests_success: number;
  cost_covered_success: number;
  cost_usd: number;
};

export type CostScatterCliProviderModelRowV1 = {
  cli_key: CliKey;
  provider_name: string;
  model: string;
  requests_success: number;
  total_cost_usd: number;
  total_duration_ms: number;
};

export type CostTopRequestRowV1 = {
  log_id: number;
  trace_id: string;
  cli_key: CliKey;
  method: string;
  path: string;
  requested_model: string | null;
  provider_id: number;
  provider_name: string;
  duration_ms: number;
  ttfb_ms: number | null;
  cost_usd: number;
  cost_multiplier: number;
  created_at: number;
};

export type CostBackfillReportV1 = {
  scanned: number;
  updated: number;
  skipped_no_model: number;
  skipped_no_usage: number;
  skipped_no_price: number;
  skipped_other: number;
  capped: boolean;
  max_rows: number;
};

export async function costSummaryV1(
  period: CostPeriod,
  input?: {
    startTs?: number | null;
    endTs?: number | null;
    cliKey?: CliKey | null;
    providerId?: number | null;
    model?: string | null;
  }
) {
  return invokeTauriOrNull<CostSummaryV1>("cost_summary_v1", {
    period,
    startTs: input?.startTs ?? null,
    endTs: input?.endTs ?? null,
    cliKey: input?.cliKey ?? null,
    providerId: input?.providerId ?? null,
    model: input?.model ?? null,
  });
}

export async function costTrendV1(
  period: CostPeriod,
  input?: {
    startTs?: number | null;
    endTs?: number | null;
    cliKey?: CliKey | null;
    providerId?: number | null;
    model?: string | null;
  }
) {
  return invokeTauriOrNull<CostTrendRowV1[]>("cost_trend_v1", {
    period,
    startTs: input?.startTs ?? null,
    endTs: input?.endTs ?? null,
    cliKey: input?.cliKey ?? null,
    providerId: input?.providerId ?? null,
    model: input?.model ?? null,
  });
}

export async function costBreakdownProviderV1(
  period: CostPeriod,
  input?: {
    startTs?: number | null;
    endTs?: number | null;
    cliKey?: CliKey | null;
    providerId?: number | null;
    model?: string | null;
    limit?: number | null;
  }
) {
  return invokeTauriOrNull<CostProviderBreakdownRowV1[]>("cost_breakdown_provider_v1", {
    period,
    startTs: input?.startTs ?? null,
    endTs: input?.endTs ?? null,
    cliKey: input?.cliKey ?? null,
    providerId: input?.providerId ?? null,
    model: input?.model ?? null,
    limit: input?.limit ?? null,
  });
}

export async function costBreakdownModelV1(
  period: CostPeriod,
  input?: {
    startTs?: number | null;
    endTs?: number | null;
    cliKey?: CliKey | null;
    providerId?: number | null;
    model?: string | null;
    limit?: number | null;
  }
) {
  return invokeTauriOrNull<CostModelBreakdownRowV1[]>("cost_breakdown_model_v1", {
    period,
    startTs: input?.startTs ?? null,
    endTs: input?.endTs ?? null,
    cliKey: input?.cliKey ?? null,
    providerId: input?.providerId ?? null,
    model: input?.model ?? null,
    limit: input?.limit ?? null,
  });
}

export async function costTopRequestsV1(
  period: CostPeriod,
  input?: {
    startTs?: number | null;
    endTs?: number | null;
    cliKey?: CliKey | null;
    providerId?: number | null;
    model?: string | null;
    limit?: number | null;
  }
) {
  return invokeTauriOrNull<CostTopRequestRowV1[]>("cost_top_requests_v1", {
    period,
    startTs: input?.startTs ?? null,
    endTs: input?.endTs ?? null,
    cliKey: input?.cliKey ?? null,
    providerId: input?.providerId ?? null,
    model: input?.model ?? null,
    limit: input?.limit ?? null,
  });
}

export async function costScatterCliProviderModelV1(
  period: CostPeriod,
  input?: {
    startTs?: number | null;
    endTs?: number | null;
    cliKey?: CliKey | null;
    providerId?: number | null;
    model?: string | null;
    limit?: number | null;
  }
) {
  return invokeTauriOrNull<CostScatterCliProviderModelRowV1[]>(
    "cost_scatter_cli_provider_model_v1",
    {
      period,
      startTs: input?.startTs ?? null,
      endTs: input?.endTs ?? null,
      cliKey: input?.cliKey ?? null,
      providerId: input?.providerId ?? null,
      model: input?.model ?? null,
      limit: input?.limit ?? null,
    }
  );
}

export async function costBackfillMissingV1(
  period: CostPeriod,
  input?: {
    startTs?: number | null;
    endTs?: number | null;
    cliKey?: CliKey | null;
    providerId?: number | null;
    model?: string | null;
    maxRows?: number | null;
  }
) {
  return invokeTauriOrNull<CostBackfillReportV1>("cost_backfill_missing_v1", {
    period,
    startTs: input?.startTs ?? null,
    endTs: input?.endTs ?? null,
    cliKey: input?.cliKey ?? null,
    providerId: input?.providerId ?? null,
    model: input?.model ?? null,
    maxRows: input?.maxRows ?? null,
  });
}
