use crate::db;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize)]
pub struct UsageSummary {
    pub requests_total: i64,
    pub requests_with_usage: i64,
    pub requests_success: i64,
    pub requests_failed: i64,
    pub avg_duration_ms: Option<i64>,
    pub avg_ttfb_ms: Option<i64>,
    pub avg_output_tokens_per_second: Option<f64>,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub io_total_tokens: i64,
    pub total_tokens: i64,
    pub cache_read_input_tokens: i64,
    pub cache_creation_input_tokens: i64,
    pub cache_creation_5m_input_tokens: i64,
    pub cache_creation_1h_input_tokens: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct UsageProviderRow {
    pub cli_key: String,
    pub provider_id: i64,
    pub provider_name: String,
    pub requests_total: i64,
    pub requests_success: i64,
    pub requests_failed: i64,
    pub avg_duration_ms: Option<i64>,
    pub avg_ttfb_ms: Option<i64>,
    pub avg_output_tokens_per_second: Option<f64>,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub total_tokens: i64,
    pub cache_read_input_tokens: i64,
    pub cache_creation_input_tokens: i64,
    pub cache_creation_5m_input_tokens: i64,
    pub cache_creation_1h_input_tokens: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct UsageDayRow {
    pub day: String,
    pub requests_total: i64,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub total_tokens: i64,
    pub cache_read_input_tokens: i64,
    pub cache_creation_input_tokens: i64,
    pub cache_creation_5m_input_tokens: i64,
    pub cache_creation_1h_input_tokens: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct UsageHourlyRow {
    pub day: String,
    pub hour: i64,
    pub requests_total: i64,
    pub requests_with_usage: i64,
    pub requests_success: i64,
    pub requests_failed: i64,
    pub total_tokens: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct UsageLeaderboardRow {
    pub key: String,
    pub name: String,
    pub requests_total: i64,
    pub requests_success: i64,
    pub requests_failed: i64,
    pub total_tokens: i64,
    pub io_total_tokens: i64,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cache_creation_input_tokens: i64,
    pub cache_read_input_tokens: i64,
    pub avg_duration_ms: Option<i64>,
    pub avg_ttfb_ms: Option<i64>,
    pub avg_output_tokens_per_second: Option<f64>,
}

#[derive(Debug, Clone, Copy)]
enum UsageRange {
    Today,
    Last7,
    Last30,
    Month,
    All,
}

fn parse_range(input: &str) -> Result<UsageRange, String> {
    match input {
        "today" => Ok(UsageRange::Today),
        "last7" => Ok(UsageRange::Last7),
        "last30" => Ok(UsageRange::Last30),
        "month" => Ok(UsageRange::Month),
        "all" => Ok(UsageRange::All),
        _ => Err(format!("SEC_INVALID_INPUT: unknown range={input}")),
    }
}

#[derive(Debug, Clone, Copy)]
enum UsageScopeV2 {
    Cli,
    Provider,
    Model,
}

fn parse_scope_v2(input: &str) -> Result<UsageScopeV2, String> {
    match input {
        "cli" => Ok(UsageScopeV2::Cli),
        "provider" => Ok(UsageScopeV2::Provider),
        "model" => Ok(UsageScopeV2::Model),
        _ => Err(format!("SEC_INVALID_INPUT: unknown scope={input}")),
    }
}

#[derive(Debug, Clone, Copy)]
enum UsagePeriodV2 {
    Daily,
    Weekly,
    Monthly,
    AllTime,
    Custom,
}

fn parse_period_v2(input: &str) -> Result<UsagePeriodV2, String> {
    match input {
        "daily" => Ok(UsagePeriodV2::Daily),
        "weekly" => Ok(UsagePeriodV2::Weekly),
        "monthly" => Ok(UsagePeriodV2::Monthly),
        "allTime" | "all_time" | "all" => Ok(UsagePeriodV2::AllTime),
        "custom" => Ok(UsagePeriodV2::Custom),
        _ => Err(format!("SEC_INVALID_INPUT: unknown period={input}")),
    }
}

fn compute_bounds_v2(
    conn: &Connection,
    period: UsagePeriodV2,
    start_ts: Option<i64>,
    end_ts: Option<i64>,
) -> Result<(Option<i64>, Option<i64>), String> {
    match period {
        UsagePeriodV2::Daily => Ok((compute_start_ts(conn, UsageRange::Today)?, None)),
        UsagePeriodV2::Weekly => Ok((compute_start_ts(conn, UsageRange::Last7)?, None)),
        UsagePeriodV2::Monthly => Ok((compute_start_ts(conn, UsageRange::Month)?, None)),
        UsagePeriodV2::AllTime => Ok((None, None)),
        UsagePeriodV2::Custom => {
            let start_ts = start_ts
                .ok_or_else(|| "SEC_INVALID_INPUT: custom period requires start_ts".to_string())?;
            let end_ts = end_ts
                .ok_or_else(|| "SEC_INVALID_INPUT: custom period requires end_ts".to_string())?;
            if start_ts >= end_ts {
                return Err(
                    "SEC_INVALID_INPUT: custom range requires start_ts < end_ts".to_string()
                );
            }
            Ok((Some(start_ts), Some(end_ts)))
        }
    }
}

fn validate_cli_key(cli_key: &str) -> Result<(), String> {
    match cli_key {
        "claude" | "codex" | "gemini" => Ok(()),
        _ => Err(format!("SEC_INVALID_INPUT: unknown cli_key={cli_key}")),
    }
}

fn normalize_cli_filter(cli_key: Option<&str>) -> Result<Option<&str>, String> {
    if let Some(k) = cli_key {
        validate_cli_key(k)?;
        return Ok(Some(k));
    }
    Ok(None)
}

fn compute_start_ts(conn: &Connection, range: UsageRange) -> Result<Option<i64>, String> {
    let sql = match range {
        UsageRange::All => return Ok(None),
        UsageRange::Today => {
            "SELECT CAST(strftime('%s','now','localtime','start of day','utc') AS INTEGER)"
        }
        UsageRange::Last7 => {
            "SELECT CAST(strftime('%s','now','localtime','start of day','-6 days','utc') AS INTEGER)"
        }
        UsageRange::Last30 => {
            "SELECT CAST(strftime('%s','now','localtime','start of day','-29 days','utc') AS INTEGER)"
        }
        UsageRange::Month => {
            "SELECT CAST(strftime('%s','now','localtime','start of month','utc') AS INTEGER)"
        }
    };

    let ts = conn
        .query_row(sql, [], |row| row.get::<_, i64>(0))
        .map_err(|e| format!("DB_ERROR: failed to compute range start ts: {e}"))?;

    Ok(Some(ts))
}

fn compute_start_ts_last_n_days(conn: &Connection, days: u32) -> Result<i64, String> {
    if days < 1 {
        return Err("SEC_INVALID_INPUT: days must be >= 1".to_string());
    }
    let offset_days = days.saturating_sub(1);
    let modifier = format!("-{offset_days} days");

    let ts = conn
        .query_row(
            "SELECT CAST(strftime('%s','now','localtime','start of day', ?1,'utc') AS INTEGER)",
            params![modifier],
            |row| row.get::<_, i64>(0),
        )
        .map_err(|e| format!("DB_ERROR: failed to compute last-days start ts: {e}"))?;

    Ok(ts)
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
struct ProviderKey {
    cli_key: String,
    provider_id: i64,
    provider_name: String,
}

#[derive(Default)]
struct ProviderAgg {
    requests_total: i64,
    requests_success: i64,
    requests_failed: i64,
    success_duration_ms_sum: i64,
    success_ttfb_ms_sum: i64,
    success_ttfb_ms_count: i64,
    success_generation_ms_sum: i64,
    success_output_tokens_for_rate_sum: i64,
    input_tokens: i64,
    output_tokens: i64,
    total_tokens: i64,
    cache_read_input_tokens: i64,
    cache_creation_input_tokens: i64,
    cache_creation_5m_input_tokens: i64,
    cache_creation_1h_input_tokens: i64,
}

#[derive(Default)]
struct LeaderboardAgg {
    requests_total: i64,
    requests_success: i64,
    requests_failed: i64,
    success_duration_ms_sum: i64,
    success_ttfb_ms_sum: i64,
    success_ttfb_ms_count: i64,
    success_generation_ms_sum: i64,
    success_output_tokens_for_rate_sum: i64,
    total_tokens: i64,
    input_tokens: i64,
    output_tokens: i64,
    cache_creation_input_tokens: i64,
    cache_read_input_tokens: i64,
}

#[derive(Debug, Deserialize)]
struct AttemptRow {
    provider_id: i64,
    provider_name: String,
    outcome: String,
}

fn extract_final_provider(cli_key: &str, attempts_json: &str) -> ProviderKey {
    let attempts: Vec<AttemptRow> = serde_json::from_str(attempts_json).unwrap_or_default();

    let picked = attempts
        .iter()
        .rev()
        .find(|a| a.outcome == "success")
        .or_else(|| attempts.last());

    match picked {
        Some(a) => ProviderKey {
            cli_key: cli_key.to_string(),
            provider_id: a.provider_id,
            provider_name: a.provider_name.clone(),
        },
        None => ProviderKey {
            cli_key: cli_key.to_string(),
            provider_id: 0,
            provider_name: "Unknown".to_string(),
        },
    }
}

fn has_valid_provider_key(key: &ProviderKey) -> bool {
    if key.provider_id <= 0 {
        return false;
    }
    let name = key.provider_name.trim();
    if name.is_empty() {
        return false;
    }
    if name == "Unknown" {
        return false;
    }
    true
}

fn token_total(total: Option<i64>, input: Option<i64>, output: Option<i64>) -> i64 {
    if let Some(t) = total {
        return t;
    }
    input.unwrap_or(0).saturating_add(output.unwrap_or(0))
}

fn is_success(status: Option<i64>, error_code: Option<&str>) -> bool {
    status.is_some_and(|v| (200..300).contains(&v)) && error_code.is_none()
}

fn is_cache_read_subset_cli(cli_key: &str) -> bool {
    matches!(cli_key, "codex" | "gemini")
}

fn effective_input_tokens(
    cli_key: &str,
    raw_input_tokens: i64,
    cache_read_input_tokens: i64,
) -> i64 {
    let raw_input_tokens = raw_input_tokens.max(0);
    let cache_read_input_tokens = cache_read_input_tokens.max(0);

    if is_cache_read_subset_cli(cli_key) {
        (raw_input_tokens.saturating_sub(cache_read_input_tokens)).max(0)
    } else {
        raw_input_tokens
    }
}

fn effective_total_tokens(
    effective_input_tokens: i64,
    output_tokens: i64,
    cache_creation_input_tokens: i64,
    cache_read_input_tokens: i64,
) -> i64 {
    effective_input_tokens
        .max(0)
        .saturating_add(output_tokens.max(0))
        .saturating_add(cache_creation_input_tokens.max(0))
        .saturating_add(cache_read_input_tokens.max(0))
}

const SQL_EFFECTIVE_INPUT_TOKENS_EXPR: &str = "CASE WHEN cli_key IN ('codex','gemini') THEN MAX(COALESCE(input_tokens, 0) - COALESCE(cache_read_input_tokens, 0), 0) ELSE COALESCE(input_tokens, 0) END";

fn sql_effective_total_tokens_expr() -> String {
    format!(
        "({effective_input_expr}) + COALESCE(output_tokens, 0) + COALESCE(cache_creation_input_tokens, 0) + COALESCE(cache_read_input_tokens, 0)",
        effective_input_expr = SQL_EFFECTIVE_INPUT_TOKENS_EXPR
    )
}

fn summary_query(
    conn: &Connection,
    start_ts: Option<i64>,
    end_ts: Option<i64>,
    cli_key: Option<&str>,
) -> Result<UsageSummary, String> {
    let effective_input_expr = SQL_EFFECTIVE_INPUT_TOKENS_EXPR;
    let effective_total_expr = sql_effective_total_tokens_expr();
    let sql = format!(
        r#"
	SELECT
	  COUNT(*) AS requests_total,
	  SUM(
	    CASE WHEN (
      total_tokens IS NOT NULL OR
      input_tokens IS NOT NULL OR
      output_tokens IS NOT NULL OR
      cache_read_input_tokens IS NOT NULL OR
      cache_creation_input_tokens IS NOT NULL OR
      cache_creation_5m_input_tokens IS NOT NULL OR
      cache_creation_1h_input_tokens IS NOT NULL OR
      usage_json IS NOT NULL
    ) THEN 1 ELSE 0 END
  ) AS requests_with_usage,
  SUM(CASE WHEN status >= 200 AND status < 300 AND error_code IS NULL THEN 1 ELSE 0 END) AS requests_success,
  SUM(
    CASE WHEN (
      status IS NULL OR
      status < 200 OR
      status >= 300 OR
      error_code IS NOT NULL
    ) THEN 1 ELSE 0 END
	  ) AS requests_failed,
	  SUM(CASE WHEN status >= 200 AND status < 300 AND error_code IS NULL THEN duration_ms ELSE 0 END) AS success_duration_ms_sum,
	  SUM(
	    CASE WHEN (
	      status >= 200 AND status < 300 AND error_code IS NULL AND
        ttfb_ms IS NOT NULL AND
	      ttfb_ms < duration_ms
	    ) THEN ttfb_ms ELSE 0 END
	  ) AS success_ttfb_ms_sum,
	  SUM(
	    CASE WHEN (
	      status >= 200 AND status < 300 AND error_code IS NULL AND
        ttfb_ms IS NOT NULL AND
	      ttfb_ms < duration_ms
	    ) THEN 1 ELSE 0 END
	  ) AS success_ttfb_ms_count,
	  SUM(
	    CASE WHEN (
	      status >= 200 AND status < 300 AND error_code IS NULL AND
	      output_tokens IS NOT NULL AND
        ttfb_ms IS NOT NULL AND
	      ttfb_ms < duration_ms
	    ) THEN (duration_ms - ttfb_ms) ELSE 0 END
	  ) AS success_generation_ms_sum,
	  SUM(
	    CASE WHEN (
	      status >= 200 AND status < 300 AND error_code IS NULL AND
	      output_tokens IS NOT NULL AND
        ttfb_ms IS NOT NULL AND
	      ttfb_ms < duration_ms
	    ) THEN output_tokens ELSE 0 END
	  ) AS success_output_tokens_for_rate_sum,
	  SUM({effective_input_expr}) AS input_tokens,
	  SUM(COALESCE(output_tokens, 0)) AS output_tokens,
	  SUM({effective_total_expr}) AS total_tokens,
	  SUM(COALESCE(cache_read_input_tokens, 0)) AS cache_read_input_tokens,
  SUM(COALESCE(cache_creation_input_tokens, 0)) AS cache_creation_input_tokens,
  SUM(COALESCE(cache_creation_5m_input_tokens, 0)) AS cache_creation_5m_input_tokens,
  SUM(COALESCE(cache_creation_1h_input_tokens, 0)) AS cache_creation_1h_input_tokens
	FROM request_logs
	WHERE excluded_from_stats = 0
  AND (?1 IS NULL OR created_at >= ?1)
  AND (?2 IS NULL OR created_at < ?2)
	AND (?3 IS NULL OR cli_key = ?3)
	"#,
        effective_input_expr = effective_input_expr,
        effective_total_expr = effective_total_expr.as_str()
    );

    conn.query_row(&sql, params![start_ts, end_ts, cli_key], |row| {
        let requests_success = row.get::<_, Option<i64>>("requests_success")?.unwrap_or(0);
        let success_duration_ms_sum = row
            .get::<_, Option<i64>>("success_duration_ms_sum")?
            .unwrap_or(0);
        let success_ttfb_ms_sum = row
            .get::<_, Option<i64>>("success_ttfb_ms_sum")?
            .unwrap_or(0);
        let success_ttfb_ms_count = row
            .get::<_, Option<i64>>("success_ttfb_ms_count")?
            .unwrap_or(0);
        let success_generation_ms_sum = row
            .get::<_, Option<i64>>("success_generation_ms_sum")?
            .unwrap_or(0);
        let success_output_tokens_for_rate_sum = row
            .get::<_, Option<i64>>("success_output_tokens_for_rate_sum")?
            .unwrap_or(0);

        let avg_duration_ms = if requests_success > 0 {
            Some(success_duration_ms_sum / requests_success)
        } else {
            None
        };
        let avg_ttfb_ms = if success_ttfb_ms_count > 0 {
            Some(success_ttfb_ms_sum / success_ttfb_ms_count)
        } else {
            None
        };
        let avg_output_tokens_per_second = if success_generation_ms_sum > 0 {
            Some(
                success_output_tokens_for_rate_sum as f64
                    / (success_generation_ms_sum as f64 / 1000.0),
            )
        } else {
            None
        };

        let input_tokens = row.get::<_, Option<i64>>("input_tokens")?.unwrap_or(0);
        let output_tokens = row.get::<_, Option<i64>>("output_tokens")?.unwrap_or(0);
        let io_total_tokens = input_tokens.saturating_add(output_tokens);

        Ok(UsageSummary {
            requests_total: row.get::<_, i64>("requests_total")?,
            requests_with_usage: row
                .get::<_, Option<i64>>("requests_with_usage")?
                .unwrap_or(0),
            requests_success,
            requests_failed: row.get::<_, Option<i64>>("requests_failed")?.unwrap_or(0),
            avg_duration_ms,
            avg_ttfb_ms,
            avg_output_tokens_per_second,
            input_tokens,
            output_tokens,
            io_total_tokens,
            total_tokens: row.get::<_, Option<i64>>("total_tokens")?.unwrap_or(0),
            cache_read_input_tokens: row
                .get::<_, Option<i64>>("cache_read_input_tokens")?
                .unwrap_or(0),
            cache_creation_input_tokens: row
                .get::<_, Option<i64>>("cache_creation_input_tokens")?
                .unwrap_or(0),
            cache_creation_5m_input_tokens: row
                .get::<_, Option<i64>>("cache_creation_5m_input_tokens")?
                .unwrap_or(0),
            cache_creation_1h_input_tokens: row
                .get::<_, Option<i64>>("cache_creation_1h_input_tokens")?
                .unwrap_or(0),
        })
    })
    .map_err(|e| format!("DB_ERROR: failed to query usage summary: {e}"))
}

pub fn summary(
    app: &tauri::AppHandle,
    range: &str,
    cli_key: Option<&str>,
) -> Result<UsageSummary, String> {
    let conn = db::open_connection(app)?;
    let range = parse_range(range)?;
    let start_ts = compute_start_ts(&conn, range)?;
    let cli_key = normalize_cli_filter(cli_key)?;

    summary_query(&conn, start_ts, None, cli_key)
}

pub fn summary_v2(
    app: &tauri::AppHandle,
    period: &str,
    start_ts: Option<i64>,
    end_ts: Option<i64>,
    cli_key: Option<&str>,
) -> Result<UsageSummary, String> {
    let conn = db::open_connection(app)?;
    let period = parse_period_v2(period)?;
    let (start_ts, end_ts) = compute_bounds_v2(&conn, period, start_ts, end_ts)?;
    let cli_key = normalize_cli_filter(cli_key)?;
    summary_query(&conn, start_ts, end_ts, cli_key)
}

fn leaderboard_row_from_agg(key: String, name: String, agg: LeaderboardAgg) -> UsageLeaderboardRow {
    let avg_duration_ms = if agg.requests_success > 0 {
        Some(agg.success_duration_ms_sum / agg.requests_success)
    } else {
        None
    };
    let avg_ttfb_ms = if agg.success_ttfb_ms_count > 0 {
        Some(agg.success_ttfb_ms_sum / agg.success_ttfb_ms_count)
    } else {
        None
    };
    let avg_output_tokens_per_second = if agg.success_generation_ms_sum > 0 {
        Some(
            agg.success_output_tokens_for_rate_sum as f64
                / (agg.success_generation_ms_sum as f64 / 1000.0),
        )
    } else {
        None
    };

    UsageLeaderboardRow {
        key,
        name,
        requests_total: agg.requests_total,
        requests_success: agg.requests_success,
        requests_failed: agg.requests_failed,
        total_tokens: agg.total_tokens,
        io_total_tokens: agg.input_tokens.saturating_add(agg.output_tokens),
        input_tokens: agg.input_tokens,
        output_tokens: agg.output_tokens,
        cache_creation_input_tokens: agg.cache_creation_input_tokens,
        cache_read_input_tokens: agg.cache_read_input_tokens,
        avg_duration_ms,
        avg_ttfb_ms,
        avg_output_tokens_per_second,
    }
}

fn leaderboard_v2_with_conn(
    conn: &Connection,
    scope: UsageScopeV2,
    start_ts: Option<i64>,
    end_ts: Option<i64>,
    cli_key: Option<&str>,
    limit: usize,
) -> Result<Vec<UsageLeaderboardRow>, String> {
    let effective_input_expr = SQL_EFFECTIVE_INPUT_TOKENS_EXPR;
    let effective_total_expr = sql_effective_total_tokens_expr();

    let mut out: Vec<UsageLeaderboardRow> = match scope {
        UsageScopeV2::Cli => {
            let sql = format!(
                r#"
SELECT
  cli_key AS key,
  COUNT(*) AS requests_total,
  SUM(CASE WHEN status >= 200 AND status < 300 AND error_code IS NULL THEN 1 ELSE 0 END) AS requests_success,
  SUM(
    CASE WHEN (
      status IS NULL OR
      status < 200 OR
      status >= 300 OR
      error_code IS NOT NULL
    ) THEN 1 ELSE 0 END
  ) AS requests_failed,
  SUM({effective_total_expr}) AS total_tokens,
  SUM({effective_input_expr}) AS input_tokens,
  SUM(COALESCE(output_tokens, 0)) AS output_tokens,
  SUM(COALESCE(cache_creation_input_tokens, 0)) AS cache_creation_input_tokens,
  SUM(COALESCE(cache_read_input_tokens, 0)) AS cache_read_input_tokens,
  SUM(CASE WHEN status >= 200 AND status < 300 AND error_code IS NULL THEN duration_ms ELSE 0 END) AS success_duration_ms_sum,
  SUM(
    CASE WHEN (
      status >= 200 AND status < 300 AND error_code IS NULL AND
      ttfb_ms IS NOT NULL AND
      ttfb_ms < duration_ms
    ) THEN ttfb_ms ELSE 0 END
  ) AS success_ttfb_ms_sum,
  SUM(
    CASE WHEN (
      status >= 200 AND status < 300 AND error_code IS NULL AND
      ttfb_ms IS NOT NULL AND
      ttfb_ms < duration_ms
    ) THEN 1 ELSE 0 END
  ) AS success_ttfb_ms_count,
  SUM(
    CASE WHEN (
      status >= 200 AND status < 300 AND error_code IS NULL AND
      output_tokens IS NOT NULL AND
      ttfb_ms IS NOT NULL AND
      ttfb_ms < duration_ms
    ) THEN (duration_ms - ttfb_ms) ELSE 0 END
  ) AS success_generation_ms_sum,
  SUM(
    CASE WHEN (
      status >= 200 AND status < 300 AND error_code IS NULL AND
      output_tokens IS NOT NULL AND
      ttfb_ms IS NOT NULL AND
      ttfb_ms < duration_ms
    ) THEN output_tokens ELSE 0 END
  ) AS success_output_tokens_for_rate_sum
FROM request_logs
WHERE excluded_from_stats = 0
AND (?1 IS NULL OR created_at >= ?1)
AND (?2 IS NULL OR created_at < ?2)
AND (?3 IS NULL OR cli_key = ?3)
GROUP BY cli_key
"#,
                effective_input_expr = effective_input_expr,
                effective_total_expr = effective_total_expr.as_str()
            );
            let mut stmt = conn
                .prepare(&sql)
                .map_err(|e| format!("DB_ERROR: failed to prepare cli leaderboard query: {e}"))?;

            let rows = stmt
                .query_map(params![start_ts, end_ts, cli_key], |row| {
                    let key: String = row.get("key")?;
                    let agg = LeaderboardAgg {
                        requests_total: row.get("requests_total")?,
                        requests_success: row
                            .get::<_, Option<i64>>("requests_success")?
                            .unwrap_or(0),
                        requests_failed: row.get::<_, Option<i64>>("requests_failed")?.unwrap_or(0),
                        success_duration_ms_sum: row
                            .get::<_, Option<i64>>("success_duration_ms_sum")?
                            .unwrap_or(0),
                        success_ttfb_ms_sum: row
                            .get::<_, Option<i64>>("success_ttfb_ms_sum")?
                            .unwrap_or(0),
                        success_ttfb_ms_count: row
                            .get::<_, Option<i64>>("success_ttfb_ms_count")?
                            .unwrap_or(0),
                        success_generation_ms_sum: row
                            .get::<_, Option<i64>>("success_generation_ms_sum")?
                            .unwrap_or(0),
                        success_output_tokens_for_rate_sum: row
                            .get::<_, Option<i64>>("success_output_tokens_for_rate_sum")?
                            .unwrap_or(0),
                        total_tokens: row.get::<_, Option<i64>>("total_tokens")?.unwrap_or(0),
                        input_tokens: row.get::<_, Option<i64>>("input_tokens")?.unwrap_or(0),
                        output_tokens: row.get::<_, Option<i64>>("output_tokens")?.unwrap_or(0),
                        cache_creation_input_tokens: row
                            .get::<_, Option<i64>>("cache_creation_input_tokens")?
                            .unwrap_or(0),
                        cache_read_input_tokens: row
                            .get::<_, Option<i64>>("cache_read_input_tokens")?
                            .unwrap_or(0),
                    };

                    Ok(leaderboard_row_from_agg(key.clone(), key, agg))
                })
                .map_err(|e| format!("DB_ERROR: failed to run cli leaderboard query: {e}"))?;

            let mut items = Vec::new();
            for row in rows {
                items.push(row.map_err(|e| format!("DB_ERROR: failed to read cli row: {e}"))?);
            }
            items
        }
        UsageScopeV2::Model => {
            let sql = format!(
                r#"
SELECT
  COALESCE(NULLIF(requested_model, ''), 'Unknown') AS key,
  COUNT(*) AS requests_total,
  SUM(CASE WHEN status >= 200 AND status < 300 AND error_code IS NULL THEN 1 ELSE 0 END) AS requests_success,
  SUM(
    CASE WHEN (
      status IS NULL OR
      status < 200 OR
      status >= 300 OR
      error_code IS NOT NULL
    ) THEN 1 ELSE 0 END
  ) AS requests_failed,
  SUM({effective_total_expr}) AS total_tokens,
  SUM({effective_input_expr}) AS input_tokens,
  SUM(COALESCE(output_tokens, 0)) AS output_tokens,
  SUM(COALESCE(cache_creation_input_tokens, 0)) AS cache_creation_input_tokens,
  SUM(COALESCE(cache_read_input_tokens, 0)) AS cache_read_input_tokens,
  SUM(CASE WHEN status >= 200 AND status < 300 AND error_code IS NULL THEN duration_ms ELSE 0 END) AS success_duration_ms_sum,
  SUM(
    CASE WHEN (
      status >= 200 AND status < 300 AND error_code IS NULL AND
      ttfb_ms IS NOT NULL AND
      ttfb_ms < duration_ms
    ) THEN ttfb_ms ELSE 0 END
  ) AS success_ttfb_ms_sum,
  SUM(
    CASE WHEN (
      status >= 200 AND status < 300 AND error_code IS NULL AND
      ttfb_ms IS NOT NULL AND
      ttfb_ms < duration_ms
    ) THEN 1 ELSE 0 END
  ) AS success_ttfb_ms_count,
  SUM(
    CASE WHEN (
      status >= 200 AND status < 300 AND error_code IS NULL AND
      output_tokens IS NOT NULL AND
      ttfb_ms IS NOT NULL AND
      ttfb_ms < duration_ms
    ) THEN (duration_ms - ttfb_ms) ELSE 0 END
  ) AS success_generation_ms_sum,
  SUM(
    CASE WHEN (
      status >= 200 AND status < 300 AND error_code IS NULL AND
      output_tokens IS NOT NULL AND
      ttfb_ms IS NOT NULL AND
      ttfb_ms < duration_ms
    ) THEN output_tokens ELSE 0 END
  ) AS success_output_tokens_for_rate_sum
FROM request_logs
WHERE excluded_from_stats = 0
AND (?1 IS NULL OR created_at >= ?1)
AND (?2 IS NULL OR created_at < ?2)
AND (?3 IS NULL OR cli_key = ?3)
GROUP BY COALESCE(NULLIF(requested_model, ''), 'Unknown')
"#,
                effective_input_expr = effective_input_expr,
                effective_total_expr = effective_total_expr.as_str()
            );
            let mut stmt = conn
                .prepare(&sql)
                .map_err(|e| format!("DB_ERROR: failed to prepare model leaderboard query: {e}"))?;

            let rows = stmt
                .query_map(params![start_ts, end_ts, cli_key], |row| {
                    let key: String = row.get("key")?;
                    let agg = LeaderboardAgg {
                        requests_total: row.get("requests_total")?,
                        requests_success: row
                            .get::<_, Option<i64>>("requests_success")?
                            .unwrap_or(0),
                        requests_failed: row.get::<_, Option<i64>>("requests_failed")?.unwrap_or(0),
                        success_duration_ms_sum: row
                            .get::<_, Option<i64>>("success_duration_ms_sum")?
                            .unwrap_or(0),
                        success_ttfb_ms_sum: row
                            .get::<_, Option<i64>>("success_ttfb_ms_sum")?
                            .unwrap_or(0),
                        success_ttfb_ms_count: row
                            .get::<_, Option<i64>>("success_ttfb_ms_count")?
                            .unwrap_or(0),
                        success_generation_ms_sum: row
                            .get::<_, Option<i64>>("success_generation_ms_sum")?
                            .unwrap_or(0),
                        success_output_tokens_for_rate_sum: row
                            .get::<_, Option<i64>>("success_output_tokens_for_rate_sum")?
                            .unwrap_or(0),
                        total_tokens: row.get::<_, Option<i64>>("total_tokens")?.unwrap_or(0),
                        input_tokens: row.get::<_, Option<i64>>("input_tokens")?.unwrap_or(0),
                        output_tokens: row.get::<_, Option<i64>>("output_tokens")?.unwrap_or(0),
                        cache_creation_input_tokens: row
                            .get::<_, Option<i64>>("cache_creation_input_tokens")?
                            .unwrap_or(0),
                        cache_read_input_tokens: row
                            .get::<_, Option<i64>>("cache_read_input_tokens")?
                            .unwrap_or(0),
                    };

                    Ok(leaderboard_row_from_agg(key.clone(), key, agg))
                })
                .map_err(|e| format!("DB_ERROR: failed to run model leaderboard query: {e}"))?;

            let mut items = Vec::new();
            for row in rows {
                items.push(row.map_err(|e| format!("DB_ERROR: failed to read model row: {e}"))?);
            }
            items
        }
        UsageScopeV2::Provider => {
            let mut stmt = conn
                .prepare(
                    r#"
SELECT
  cli_key,
  attempts_json,
  status,
  error_code,
  duration_ms,
  ttfb_ms,
  input_tokens,
  output_tokens,
  cache_read_input_tokens,
  cache_creation_input_tokens,
  cache_creation_5m_input_tokens,
  cache_creation_1h_input_tokens
FROM request_logs
WHERE excluded_from_stats = 0
AND (?1 IS NULL OR created_at >= ?1)
AND (?2 IS NULL OR created_at < ?2)
AND (?3 IS NULL OR cli_key = ?3)
"#,
                )
                .map_err(|e| {
                    format!("DB_ERROR: failed to prepare provider leaderboard query: {e}")
                })?;

            let rows = stmt
                .query_map(params![start_ts, end_ts, cli_key], |row| {
                    let row_cli_key: String = row.get("cli_key")?;
                    let attempts_json: String = row.get("attempts_json")?;
                    let status: Option<i64> = row.get("status")?;
                    let error_code: Option<String> = row.get("error_code")?;
                    let duration_ms: i64 = row.get("duration_ms")?;
                    let ttfb_ms: Option<i64> = row.get("ttfb_ms")?;
                    let input_tokens: Option<i64> = row.get("input_tokens")?;
                    let output_tokens: Option<i64> = row.get("output_tokens")?;
                    let cache_read_input_tokens: Option<i64> =
                        row.get("cache_read_input_tokens")?;
                    let cache_creation_input_tokens: Option<i64> =
                        row.get("cache_creation_input_tokens")?;
                    let cache_creation_5m_input_tokens: Option<i64> =
                        row.get("cache_creation_5m_input_tokens")?;
                    let cache_creation_1h_input_tokens: Option<i64> =
                        row.get("cache_creation_1h_input_tokens")?;

                    let key = extract_final_provider(&row_cli_key, &attempts_json);
                    let success = is_success(status, error_code.as_deref());

                    let ttfb_ms = match ttfb_ms {
                        Some(v) if v < duration_ms => Some(v),
                        _ => None,
                    };
                    let ttfb_ms_for_rate = ttfb_ms.unwrap_or(duration_ms);
                    let generation_ms = duration_ms.saturating_sub(ttfb_ms_for_rate);
                    let (rate_generation_ms, rate_output_tokens) =
                        if success && generation_ms > 0 && output_tokens.is_some() {
                            (generation_ms, output_tokens.unwrap_or(0))
                        } else {
                            (0, 0)
                        };

                    let raw_input_tokens = input_tokens.unwrap_or(0);
                    let raw_output_tokens = output_tokens.unwrap_or(0);
                    let cache_read_input_tokens = cache_read_input_tokens.unwrap_or(0);
                    let cache_creation_input_tokens = cache_creation_input_tokens.unwrap_or(0);

                    let effective_input_tokens_value = effective_input_tokens(
                        &row_cli_key,
                        raw_input_tokens,
                        cache_read_input_tokens,
                    );
                    let effective_total_tokens_value = effective_total_tokens(
                        effective_input_tokens_value,
                        raw_output_tokens,
                        cache_creation_input_tokens,
                        cache_read_input_tokens,
                    );

                    Ok((
                        key,
                        ProviderAgg {
                            requests_total: 1,
                            requests_success: if success { 1 } else { 0 },
                            requests_failed: if success { 0 } else { 1 },
                            success_duration_ms_sum: if success { duration_ms } else { 0 },
                            success_ttfb_ms_sum: if success { ttfb_ms.unwrap_or(0) } else { 0 },
                            success_ttfb_ms_count: if success && ttfb_ms.is_some() { 1 } else { 0 },
                            success_generation_ms_sum: rate_generation_ms,
                            success_output_tokens_for_rate_sum: rate_output_tokens,
                            input_tokens: effective_input_tokens_value,
                            output_tokens: raw_output_tokens,
                            total_tokens: effective_total_tokens_value,
                            cache_read_input_tokens,
                            cache_creation_input_tokens,
                            cache_creation_5m_input_tokens: cache_creation_5m_input_tokens
                                .unwrap_or(0),
                            cache_creation_1h_input_tokens: cache_creation_1h_input_tokens
                                .unwrap_or(0),
                        },
                    ))
                })
                .map_err(|e| format!("DB_ERROR: failed to run provider leaderboard query: {e}"))?;

            let mut agg: HashMap<ProviderKey, ProviderAgg> = HashMap::new();
            for row in rows {
                let (key, add) = row.map_err(|e| {
                    format!("DB_ERROR: failed to read provider leaderboard row: {e}")
                })?;

                if !has_valid_provider_key(&key) {
                    continue;
                }

                let entry = agg.entry(key).or_default();
                entry.requests_total = entry.requests_total.saturating_add(add.requests_total);
                entry.requests_success =
                    entry.requests_success.saturating_add(add.requests_success);
                entry.requests_failed = entry.requests_failed.saturating_add(add.requests_failed);
                entry.success_duration_ms_sum = entry
                    .success_duration_ms_sum
                    .saturating_add(add.success_duration_ms_sum);
                entry.success_ttfb_ms_sum = entry
                    .success_ttfb_ms_sum
                    .saturating_add(add.success_ttfb_ms_sum);
                entry.success_ttfb_ms_count = entry
                    .success_ttfb_ms_count
                    .saturating_add(add.success_ttfb_ms_count);
                entry.success_generation_ms_sum = entry
                    .success_generation_ms_sum
                    .saturating_add(add.success_generation_ms_sum);
                entry.success_output_tokens_for_rate_sum = entry
                    .success_output_tokens_for_rate_sum
                    .saturating_add(add.success_output_tokens_for_rate_sum);
                entry.total_tokens = entry.total_tokens.saturating_add(add.total_tokens);
                entry.input_tokens = entry.input_tokens.saturating_add(add.input_tokens);
                entry.output_tokens = entry.output_tokens.saturating_add(add.output_tokens);
                entry.cache_creation_input_tokens = entry
                    .cache_creation_input_tokens
                    .saturating_add(add.cache_creation_input_tokens);
                entry.cache_read_input_tokens = entry
                    .cache_read_input_tokens
                    .saturating_add(add.cache_read_input_tokens);
            }

            agg.into_iter()
                .map(|(k, v)| {
                    let key = format!("{}:{}", k.cli_key, k.provider_id);
                    let name = format!("{}/{}", k.cli_key, k.provider_name);
                    let agg = LeaderboardAgg {
                        requests_total: v.requests_total,
                        requests_success: v.requests_success,
                        requests_failed: v.requests_failed,
                        success_duration_ms_sum: v.success_duration_ms_sum,
                        success_ttfb_ms_sum: v.success_ttfb_ms_sum,
                        success_ttfb_ms_count: v.success_ttfb_ms_count,
                        success_generation_ms_sum: v.success_generation_ms_sum,
                        success_output_tokens_for_rate_sum: v.success_output_tokens_for_rate_sum,
                        total_tokens: v.total_tokens,
                        input_tokens: v.input_tokens,
                        output_tokens: v.output_tokens,
                        cache_creation_input_tokens: v.cache_creation_input_tokens,
                        cache_read_input_tokens: v.cache_read_input_tokens,
                    };
                    leaderboard_row_from_agg(key, name, agg)
                })
                .collect()
        }
    };

    out.sort_by(|a, b| {
        b.requests_total
            .cmp(&a.requests_total)
            .then_with(|| b.total_tokens.cmp(&a.total_tokens))
            .then_with(|| a.name.cmp(&b.name))
            .then_with(|| a.key.cmp(&b.key))
    });
    out.truncate(limit.clamp(1, 200));
    Ok(out)
}

pub fn leaderboard_v2(
    app: &tauri::AppHandle,
    scope: &str,
    period: &str,
    start_ts: Option<i64>,
    end_ts: Option<i64>,
    cli_key: Option<&str>,
    limit: usize,
) -> Result<Vec<UsageLeaderboardRow>, String> {
    let conn = db::open_connection(app)?;
    let scope = parse_scope_v2(scope)?;
    let period = parse_period_v2(period)?;
    let (start_ts, end_ts) = compute_bounds_v2(&conn, period, start_ts, end_ts)?;
    let cli_key = normalize_cli_filter(cli_key)?;
    leaderboard_v2_with_conn(&conn, scope, start_ts, end_ts, cli_key, limit)
}

pub fn leaderboard_provider(
    app: &tauri::AppHandle,
    range: &str,
    cli_key: Option<&str>,
    limit: usize,
) -> Result<Vec<UsageProviderRow>, String> {
    let conn = db::open_connection(app)?;
    let range = parse_range(range)?;
    let start_ts = compute_start_ts(&conn, range)?;
    let cli_key = normalize_cli_filter(cli_key)?;

    let mut stmt = conn
        .prepare(
            r#"
	SELECT
	  cli_key,
	  attempts_json,
	  status,
	  error_code,
	  duration_ms,
	  ttfb_ms,
	  input_tokens,
	  output_tokens,
	  total_tokens,
	  cache_read_input_tokens,
	  cache_creation_input_tokens,
  cache_creation_5m_input_tokens,
  cache_creation_1h_input_tokens
FROM request_logs
WHERE excluded_from_stats = 0
AND (?1 IS NULL OR created_at >= ?1)
AND (?2 IS NULL OR cli_key = ?2)
"#,
        )
        .map_err(|e| format!("DB_ERROR: failed to prepare provider leaderboard query: {e}"))?;

    let rows = stmt
        .query_map(params![start_ts, cli_key], |row| {
            let row_cli_key: String = row.get("cli_key")?;
            let attempts_json: String = row.get("attempts_json")?;
            let status: Option<i64> = row.get("status")?;
            let error_code: Option<String> = row.get("error_code")?;
            let duration_ms: i64 = row.get("duration_ms")?;
            let ttfb_ms: Option<i64> = row.get("ttfb_ms")?;

            let input_tokens: Option<i64> = row.get("input_tokens")?;
            let output_tokens: Option<i64> = row.get("output_tokens")?;
            let total_tokens: Option<i64> = row.get("total_tokens")?;
            let cache_read_input_tokens: Option<i64> = row.get("cache_read_input_tokens")?;
            let cache_creation_input_tokens: Option<i64> =
                row.get("cache_creation_input_tokens")?;
            let cache_creation_5m_input_tokens: Option<i64> =
                row.get("cache_creation_5m_input_tokens")?;
            let cache_creation_1h_input_tokens: Option<i64> =
                row.get("cache_creation_1h_input_tokens")?;

            let key = extract_final_provider(&row_cli_key, &attempts_json);
            let success = is_success(status, error_code.as_deref());

            let ttfb_ms = match ttfb_ms {
                Some(v) if v < duration_ms => Some(v),
                _ => None,
            };
            let ttfb_ms_for_rate = ttfb_ms.unwrap_or(duration_ms);
            let generation_ms = duration_ms.saturating_sub(ttfb_ms_for_rate);
            let (rate_generation_ms, rate_output_tokens) =
                if success && generation_ms > 0 && output_tokens.is_some() {
                    (generation_ms, output_tokens.unwrap_or(0))
                } else {
                    (0, 0)
                };

            Ok((
                key,
                ProviderAgg {
                    requests_total: 1,
                    requests_success: if success { 1 } else { 0 },
                    requests_failed: if success { 0 } else { 1 },
                    success_duration_ms_sum: if success { duration_ms } else { 0 },
                    success_ttfb_ms_sum: if success { ttfb_ms.unwrap_or(0) } else { 0 },
                    success_ttfb_ms_count: if success && ttfb_ms.is_some() { 1 } else { 0 },
                    success_generation_ms_sum: rate_generation_ms,
                    success_output_tokens_for_rate_sum: rate_output_tokens,
                    input_tokens: input_tokens.unwrap_or(0),
                    output_tokens: output_tokens.unwrap_or(0),
                    total_tokens: token_total(total_tokens, input_tokens, output_tokens),
                    cache_read_input_tokens: cache_read_input_tokens.unwrap_or(0),
                    cache_creation_input_tokens: cache_creation_input_tokens.unwrap_or(0),
                    cache_creation_5m_input_tokens: cache_creation_5m_input_tokens.unwrap_or(0),
                    cache_creation_1h_input_tokens: cache_creation_1h_input_tokens.unwrap_or(0),
                },
            ))
        })
        .map_err(|e| format!("DB_ERROR: failed to run provider leaderboard query: {e}"))?;

    let mut agg: HashMap<ProviderKey, ProviderAgg> = HashMap::new();
    for row in rows {
        let (key, add) =
            row.map_err(|e| format!("DB_ERROR: failed to read provider leaderboard row: {e}"))?;

        if !has_valid_provider_key(&key) {
            continue;
        }

        let entry = agg.entry(key).or_default();
        entry.requests_total = entry.requests_total.saturating_add(add.requests_total);
        entry.requests_success = entry.requests_success.saturating_add(add.requests_success);
        entry.requests_failed = entry.requests_failed.saturating_add(add.requests_failed);
        entry.success_duration_ms_sum = entry
            .success_duration_ms_sum
            .saturating_add(add.success_duration_ms_sum);
        entry.success_ttfb_ms_sum = entry
            .success_ttfb_ms_sum
            .saturating_add(add.success_ttfb_ms_sum);
        entry.success_ttfb_ms_count = entry
            .success_ttfb_ms_count
            .saturating_add(add.success_ttfb_ms_count);
        entry.success_generation_ms_sum = entry
            .success_generation_ms_sum
            .saturating_add(add.success_generation_ms_sum);
        entry.success_output_tokens_for_rate_sum = entry
            .success_output_tokens_for_rate_sum
            .saturating_add(add.success_output_tokens_for_rate_sum);
        entry.input_tokens = entry.input_tokens.saturating_add(add.input_tokens);
        entry.output_tokens = entry.output_tokens.saturating_add(add.output_tokens);
        entry.total_tokens = entry.total_tokens.saturating_add(add.total_tokens);
        entry.cache_read_input_tokens = entry
            .cache_read_input_tokens
            .saturating_add(add.cache_read_input_tokens);
        entry.cache_creation_input_tokens = entry
            .cache_creation_input_tokens
            .saturating_add(add.cache_creation_input_tokens);
        entry.cache_creation_5m_input_tokens = entry
            .cache_creation_5m_input_tokens
            .saturating_add(add.cache_creation_5m_input_tokens);
        entry.cache_creation_1h_input_tokens = entry
            .cache_creation_1h_input_tokens
            .saturating_add(add.cache_creation_1h_input_tokens);
    }

    let mut out: Vec<UsageProviderRow> = agg
        .into_iter()
        .map(|(k, v)| UsageProviderRow {
            cli_key: k.cli_key,
            provider_id: k.provider_id,
            provider_name: k.provider_name,
            requests_total: v.requests_total,
            requests_success: v.requests_success,
            requests_failed: v.requests_failed,
            avg_duration_ms: if v.requests_success > 0 {
                Some(v.success_duration_ms_sum / v.requests_success)
            } else {
                None
            },
            avg_ttfb_ms: if v.success_ttfb_ms_count > 0 {
                Some(v.success_ttfb_ms_sum / v.success_ttfb_ms_count)
            } else {
                None
            },
            avg_output_tokens_per_second: if v.success_generation_ms_sum > 0 {
                Some(
                    v.success_output_tokens_for_rate_sum as f64
                        / (v.success_generation_ms_sum as f64 / 1000.0),
                )
            } else {
                None
            },
            input_tokens: v.input_tokens,
            output_tokens: v.output_tokens,
            total_tokens: v.total_tokens,
            cache_read_input_tokens: v.cache_read_input_tokens,
            cache_creation_input_tokens: v.cache_creation_input_tokens,
            cache_creation_5m_input_tokens: v.cache_creation_5m_input_tokens,
            cache_creation_1h_input_tokens: v.cache_creation_1h_input_tokens,
        })
        .collect();

    out.sort_by(|a, b| {
        b.total_tokens
            .cmp(&a.total_tokens)
            .then_with(|| b.requests_total.cmp(&a.requests_total))
            .then_with(|| a.cli_key.cmp(&b.cli_key))
            .then_with(|| a.provider_name.cmp(&b.provider_name))
    });

    out.truncate(limit.max(1));
    Ok(out)
}

pub fn leaderboard_day(
    app: &tauri::AppHandle,
    range: &str,
    cli_key: Option<&str>,
    limit: usize,
) -> Result<Vec<UsageDayRow>, String> {
    let conn = db::open_connection(app)?;
    let range = parse_range(range)?;
    let start_ts = compute_start_ts(&conn, range)?;
    let cli_key = normalize_cli_filter(cli_key)?;

    let mut stmt = conn
        .prepare(
            r#"
SELECT
  strftime('%Y-%m-%d', created_at, 'unixepoch', 'localtime') AS day,
  COUNT(*) AS requests_total,
  SUM(COALESCE(input_tokens, 0)) AS input_tokens,
  SUM(COALESCE(output_tokens, 0)) AS output_tokens,
  SUM(COALESCE(total_tokens, COALESCE(input_tokens, 0) + COALESCE(output_tokens, 0))) AS total_tokens,
  SUM(COALESCE(cache_read_input_tokens, 0)) AS cache_read_input_tokens,
  SUM(COALESCE(cache_creation_input_tokens, 0)) AS cache_creation_input_tokens,
  SUM(COALESCE(cache_creation_5m_input_tokens, 0)) AS cache_creation_5m_input_tokens,
  SUM(COALESCE(cache_creation_1h_input_tokens, 0)) AS cache_creation_1h_input_tokens
FROM request_logs
WHERE excluded_from_stats = 0
AND (?1 IS NULL OR created_at >= ?1)
AND (?2 IS NULL OR cli_key = ?2)
GROUP BY day
ORDER BY total_tokens DESC, day DESC
LIMIT ?3
"#,
        )
        .map_err(|e| format!("DB_ERROR: failed to prepare day leaderboard query: {e}"))?;

    let rows = stmt
        .query_map(params![start_ts, cli_key, limit as i64], |row| {
            Ok(UsageDayRow {
                day: row.get("day")?,
                requests_total: row.get("requests_total")?,
                input_tokens: row.get::<_, Option<i64>>("input_tokens")?.unwrap_or(0),
                output_tokens: row.get::<_, Option<i64>>("output_tokens")?.unwrap_or(0),
                total_tokens: row.get::<_, Option<i64>>("total_tokens")?.unwrap_or(0),
                cache_read_input_tokens: row
                    .get::<_, Option<i64>>("cache_read_input_tokens")?
                    .unwrap_or(0),
                cache_creation_input_tokens: row
                    .get::<_, Option<i64>>("cache_creation_input_tokens")?
                    .unwrap_or(0),
                cache_creation_5m_input_tokens: row
                    .get::<_, Option<i64>>("cache_creation_5m_input_tokens")?
                    .unwrap_or(0),
                cache_creation_1h_input_tokens: row
                    .get::<_, Option<i64>>("cache_creation_1h_input_tokens")?
                    .unwrap_or(0),
            })
        })
        .map_err(|e| format!("DB_ERROR: failed to run day leaderboard query: {e}"))?;

    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(|e| format!("DB_ERROR: failed to read day row: {e}"))?);
    }
    Ok(out)
}

pub fn hourly_series(app: &tauri::AppHandle, days: u32) -> Result<Vec<UsageHourlyRow>, String> {
    let conn = db::open_connection(app)?;
    let days = days.clamp(1, 60);
    let start_ts = compute_start_ts_last_n_days(&conn, days)?;

    let mut stmt = conn
        .prepare(
            r#"
SELECT
  strftime('%Y-%m-%d', created_at, 'unixepoch', 'localtime') AS day,
  CAST(strftime('%H', created_at, 'unixepoch', 'localtime') AS INTEGER) AS hour,
  COUNT(*) AS requests_total,
  SUM(
    CASE WHEN (
      total_tokens IS NOT NULL OR
      input_tokens IS NOT NULL OR
      output_tokens IS NOT NULL OR
      cache_read_input_tokens IS NOT NULL OR
      cache_creation_input_tokens IS NOT NULL OR
      cache_creation_5m_input_tokens IS NOT NULL OR
      cache_creation_1h_input_tokens IS NOT NULL OR
      usage_json IS NOT NULL
    ) THEN 1 ELSE 0 END
  ) AS requests_with_usage,
  SUM(CASE WHEN status >= 200 AND status < 300 AND error_code IS NULL THEN 1 ELSE 0 END) AS requests_success,
  SUM(
    CASE WHEN (
      status IS NULL OR
      status < 200 OR
      status >= 300 OR
      error_code IS NOT NULL
    ) THEN 1 ELSE 0 END
  ) AS requests_failed,
  SUM(COALESCE(total_tokens, COALESCE(input_tokens, 0) + COALESCE(output_tokens, 0))) AS total_tokens
FROM request_logs
WHERE excluded_from_stats = 0
AND created_at >= ?1
GROUP BY day, hour
ORDER BY day ASC, hour ASC
"#,
        )
        .map_err(|e| format!("DB_ERROR: failed to prepare hourly series query: {e}"))?;

    let rows = stmt
        .query_map(params![start_ts], |row| {
            Ok(UsageHourlyRow {
                day: row.get("day")?,
                hour: row.get("hour")?,
                requests_total: row.get("requests_total")?,
                requests_with_usage: row
                    .get::<_, Option<i64>>("requests_with_usage")?
                    .unwrap_or(0),
                requests_success: row.get::<_, Option<i64>>("requests_success")?.unwrap_or(0),
                requests_failed: row.get::<_, Option<i64>>("requests_failed")?.unwrap_or(0),
                total_tokens: row.get::<_, Option<i64>>("total_tokens")?.unwrap_or(0),
            })
        })
        .map_err(|e| format!("DB_ERROR: failed to run hourly series query: {e}"))?;

    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(|e| format!("DB_ERROR: failed to read hourly row: {e}"))?);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_conn() -> Connection {
        let conn = Connection::open_in_memory().expect("open in-memory sqlite");
        conn.execute_batch(
            r#"
CREATE TABLE request_logs (
  cli_key TEXT NOT NULL,
  attempts_json TEXT NOT NULL,
  requested_model TEXT,
  status INTEGER,
  error_code TEXT,
  duration_ms INTEGER NOT NULL,
  ttfb_ms INTEGER,
  input_tokens INTEGER,
  output_tokens INTEGER,
  total_tokens INTEGER,
  cache_read_input_tokens INTEGER,
  cache_creation_input_tokens INTEGER,
  cache_creation_5m_input_tokens INTEGER,
  cache_creation_1h_input_tokens INTEGER,
  usage_json TEXT,
  excluded_from_stats INTEGER NOT NULL DEFAULT 0,
  created_at INTEGER NOT NULL
);
"#,
        )
        .expect("create schema");
        conn
    }

    #[test]
    fn v2_cache_rate_denominator_aligns_across_clis() {
        let conn = setup_conn();

        // Codex/Gemini: cache_read_input_tokens is a subset of input_tokens.
        conn.execute(
            r#"
INSERT INTO request_logs (
  cli_key,
  attempts_json,
  requested_model,
  status,
  error_code,
  duration_ms,
  ttfb_ms,
  input_tokens,
  output_tokens,
  total_tokens,
  cache_read_input_tokens,
  cache_creation_input_tokens,
  cache_creation_5m_input_tokens,
  cache_creation_1h_input_tokens,
  usage_json,
  excluded_from_stats,
  created_at
) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17);
"#,
            params![
                "codex",
                r#"[{"provider_id":123,"provider_name":"OpenAI","outcome":"success"}]"#,
                "gpt-test",
                200,
                Option::<String>::None,
                1000,
                100,
                100,
                10,
                999,
                30,
                0,
                0,
                0,
                Option::<String>::None,
                0,
                1000
            ],
        )
        .expect("insert codex");

        conn.execute(
            r#"
INSERT INTO request_logs (
  cli_key,
  attempts_json,
  requested_model,
  status,
  error_code,
  duration_ms,
  ttfb_ms,
  input_tokens,
  output_tokens,
  total_tokens,
  cache_read_input_tokens,
  cache_creation_input_tokens,
  cache_creation_5m_input_tokens,
  cache_creation_1h_input_tokens,
  usage_json,
  excluded_from_stats,
  created_at
) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17);
"#,
            params![
                "gemini",
                r#"[{"provider_id":456,"provider_name":"GeminiUpstream","outcome":"success"}]"#,
                "gemini-test",
                200,
                Option::<String>::None,
                1000,
                100,
                200,
                20,
                0,
                50,
                0,
                0,
                0,
                Option::<String>::None,
                0,
                1000
            ],
        )
        .expect("insert gemini");

        // Claude: cache_read/cache_creation are additional buckets (not a subset of input_tokens).
        conn.execute(
            r#"
INSERT INTO request_logs (
  cli_key,
  attempts_json,
  requested_model,
  status,
  error_code,
  duration_ms,
  ttfb_ms,
  input_tokens,
  output_tokens,
  total_tokens,
  cache_read_input_tokens,
  cache_creation_input_tokens,
  cache_creation_5m_input_tokens,
  cache_creation_1h_input_tokens,
  usage_json,
  excluded_from_stats,
  created_at
) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17);
"#,
            params![
                "claude",
                r#"[{"provider_id":789,"provider_name":"ClaudeUpstream","outcome":"success"}]"#,
                "claude-test",
                200,
                Option::<String>::None,
                1000,
                100,
                300,
                30,
                Option::<i64>::None,
                40,
                25,
                0,
                0,
                Option::<String>::None,
                0,
                1000
            ],
        )
        .expect("insert claude");

        let summary = summary_query(&conn, None, None, None).expect("summary_query");
        assert_eq!(summary.requests_total, 3);
        assert_eq!(summary.input_tokens, 520);
        assert_eq!(summary.output_tokens, 60);
        assert_eq!(summary.io_total_tokens, 580);
        assert_eq!(summary.cache_read_input_tokens, 120);
        assert_eq!(summary.cache_creation_input_tokens, 25);
        assert_eq!(summary.total_tokens, 725);

        let rows = leaderboard_v2_with_conn(&conn, UsageScopeV2::Provider, None, None, None, 50)
            .expect("leaderboard_v2_with_conn");
        assert_eq!(rows.len(), 3);

        let by_key: std::collections::HashMap<String, UsageLeaderboardRow> =
            rows.into_iter().map(|row| (row.key.clone(), row)).collect();

        let codex = by_key.get("codex:123").expect("codex row");
        assert_eq!(codex.input_tokens, 70);
        assert_eq!(codex.output_tokens, 10);
        assert_eq!(codex.io_total_tokens, 80);
        assert_eq!(codex.cache_read_input_tokens, 30);
        assert_eq!(codex.cache_creation_input_tokens, 0);
        assert_eq!(codex.total_tokens, 110);

        let gemini = by_key.get("gemini:456").expect("gemini row");
        assert_eq!(gemini.input_tokens, 150);
        assert_eq!(gemini.output_tokens, 20);
        assert_eq!(gemini.io_total_tokens, 170);
        assert_eq!(gemini.cache_read_input_tokens, 50);
        assert_eq!(gemini.cache_creation_input_tokens, 0);
        assert_eq!(gemini.total_tokens, 220);

        let claude = by_key.get("claude:789").expect("claude row");
        assert_eq!(claude.input_tokens, 300);
        assert_eq!(claude.output_tokens, 30);
        assert_eq!(claude.io_total_tokens, 330);
        assert_eq!(claude.cache_read_input_tokens, 40);
        assert_eq!(claude.cache_creation_input_tokens, 25);
        assert_eq!(claude.total_tokens, 395);
    }
}
