use crate::db;
use rusqlite::{params, Connection, OptionalExtension};

use super::{
    compute_bounds_v2, extract_final_provider, has_valid_provider_key, normalize_cli_filter,
    parse_period_v2, parse_scope_v2, sql_effective_input_tokens_expr_with_alias,
    sql_effective_total_tokens_expr, sql_effective_total_tokens_expr_with_alias, ProviderAgg,
    ProviderKey, UsageLeaderboardRow, UsageScopeV2, SQL_EFFECTIVE_INPUT_TOKENS_EXPR,
};

pub(super) fn leaderboard_v2_with_conn(
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
	  SUM(
	    CASE WHEN (
	      status >= 200 AND status < 300 AND error_code IS NULL AND
	      cost_usd_femto IS NOT NULL AND cost_usd_femto > 0
	    ) THEN 1 ELSE 0 END
	  ) AS cost_covered_success,
	  SUM(
	    CASE WHEN (
	      status >= 200 AND status < 300 AND error_code IS NULL AND
	      cost_usd_femto IS NOT NULL AND cost_usd_femto > 0
	    ) THEN cost_usd_femto ELSE 0 END
	  ) AS total_cost_usd_femto,
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
                    let agg = ProviderAgg {
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
                        cache_creation_5m_input_tokens: 0,
                        cache_creation_1h_input_tokens: 0,
                        cost_covered_success: row
                            .get::<_, Option<i64>>("cost_covered_success")?
                            .unwrap_or(0),
                        total_cost_usd_femto: row
                            .get::<_, Option<i64>>("total_cost_usd_femto")?
                            .unwrap_or(0),
                    };

                    Ok(agg.into_leaderboard_row(key.clone(), key))
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
	  SUM(
	    CASE WHEN (
	      status >= 200 AND status < 300 AND error_code IS NULL AND
	      cost_usd_femto IS NOT NULL AND cost_usd_femto > 0
	    ) THEN 1 ELSE 0 END
	  ) AS cost_covered_success,
	  SUM(
	    CASE WHEN (
	      status >= 200 AND status < 300 AND error_code IS NULL AND
	      cost_usd_femto IS NOT NULL AND cost_usd_femto > 0
	    ) THEN cost_usd_femto ELSE 0 END
	  ) AS total_cost_usd_femto,
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
                    let agg = ProviderAgg {
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
                        cache_creation_5m_input_tokens: 0,
                        cache_creation_1h_input_tokens: 0,
                        cost_covered_success: row
                            .get::<_, Option<i64>>("cost_covered_success")?
                            .unwrap_or(0),
                        total_cost_usd_femto: row
                            .get::<_, Option<i64>>("total_cost_usd_femto")?
                            .unwrap_or(0),
                    };

                    Ok(agg.into_leaderboard_row(key.clone(), key))
                })
                .map_err(|e| format!("DB_ERROR: failed to run model leaderboard query: {e}"))?;

            let mut items = Vec::new();
            for row in rows {
                items.push(row.map_err(|e| format!("DB_ERROR: failed to read model row: {e}"))?);
            }
            items
        }
        UsageScopeV2::Provider => {
            let effective_input_expr = sql_effective_input_tokens_expr_with_alias("r");
            let effective_total_expr = sql_effective_total_tokens_expr_with_alias("r");

            let sql = format!(
                r#"
SELECT
  r.cli_key AS cli_key,
  r.final_provider_id AS provider_id,
  MAX(p.name) AS provider_name,
  COUNT(*) AS requests_total,
  SUM(CASE WHEN r.status >= 200 AND r.status < 300 AND r.error_code IS NULL THEN 1 ELSE 0 END) AS requests_success,
  SUM(
    CASE WHEN (
      r.status IS NULL OR
      r.status < 200 OR
      r.status >= 300 OR
      r.error_code IS NOT NULL
    ) THEN 1 ELSE 0 END
  ) AS requests_failed,
  SUM({effective_total_expr}) AS total_tokens,
  SUM({effective_input_expr}) AS input_tokens,
  SUM(COALESCE(r.output_tokens, 0)) AS output_tokens,
  SUM(COALESCE(r.cache_creation_input_tokens, 0)) AS cache_creation_input_tokens,
  SUM(COALESCE(r.cache_read_input_tokens, 0)) AS cache_read_input_tokens,
  SUM(COALESCE(r.cache_creation_5m_input_tokens, 0)) AS cache_creation_5m_input_tokens,
  SUM(COALESCE(r.cache_creation_1h_input_tokens, 0)) AS cache_creation_1h_input_tokens,
  SUM(
    CASE WHEN (
      r.status >= 200 AND r.status < 300 AND r.error_code IS NULL AND
      r.cost_usd_femto IS NOT NULL AND r.cost_usd_femto > 0
    ) THEN 1 ELSE 0 END
  ) AS cost_covered_success,
  SUM(
    CASE WHEN (
      r.status >= 200 AND r.status < 300 AND r.error_code IS NULL AND
      r.cost_usd_femto IS NOT NULL AND r.cost_usd_femto > 0
    ) THEN r.cost_usd_femto ELSE 0 END
  ) AS total_cost_usd_femto,
  SUM(CASE WHEN r.status >= 200 AND r.status < 300 AND r.error_code IS NULL THEN r.duration_ms ELSE 0 END) AS success_duration_ms_sum,
  SUM(
    CASE WHEN (
      r.status >= 200 AND r.status < 300 AND r.error_code IS NULL AND
      r.ttfb_ms IS NOT NULL AND
      r.ttfb_ms < r.duration_ms
    ) THEN r.ttfb_ms ELSE 0 END
  ) AS success_ttfb_ms_sum,
  SUM(
    CASE WHEN (
      r.status >= 200 AND r.status < 300 AND r.error_code IS NULL AND
      r.ttfb_ms IS NOT NULL AND
      r.ttfb_ms < r.duration_ms
    ) THEN 1 ELSE 0 END
  ) AS success_ttfb_ms_count,
  SUM(
    CASE WHEN (
      r.status >= 200 AND r.status < 300 AND r.error_code IS NULL AND
      r.output_tokens IS NOT NULL AND
      r.ttfb_ms IS NOT NULL AND
      r.ttfb_ms < r.duration_ms
    ) THEN (r.duration_ms - r.ttfb_ms) ELSE 0 END
  ) AS success_generation_ms_sum,
  SUM(
    CASE WHEN (
      r.status >= 200 AND r.status < 300 AND r.error_code IS NULL AND
      r.output_tokens IS NOT NULL AND
      r.ttfb_ms IS NOT NULL AND
      r.ttfb_ms < r.duration_ms
    ) THEN r.output_tokens ELSE 0 END
  ) AS success_output_tokens_for_rate_sum
FROM request_logs r
LEFT JOIN providers p ON p.id = r.final_provider_id
WHERE r.excluded_from_stats = 0
AND r.final_provider_id IS NOT NULL
AND r.final_provider_id > 0
AND (?1 IS NULL OR r.created_at >= ?1)
AND (?2 IS NULL OR r.created_at < ?2)
AND (?3 IS NULL OR r.cli_key = ?3)
GROUP BY r.cli_key, r.final_provider_id
"#,
                effective_input_expr = effective_input_expr,
                effective_total_expr = effective_total_expr
            );

            let mut stmt = conn.prepare(&sql).map_err(|e| {
                format!("DB_ERROR: failed to prepare provider leaderboard query: {e}")
            })?;

            let rows = stmt
                .query_map(params![start_ts, end_ts, cli_key], |row| {
                    let cli_key: String = row.get("cli_key")?;
                    let provider_id: i64 = row.get("provider_id")?;
                    let provider_name: Option<String> = row.get("provider_name")?;

                    let agg = ProviderAgg {
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
                        cache_creation_5m_input_tokens: row
                            .get::<_, Option<i64>>("cache_creation_5m_input_tokens")?
                            .unwrap_or(0),
                        cache_creation_1h_input_tokens: row
                            .get::<_, Option<i64>>("cache_creation_1h_input_tokens")?
                            .unwrap_or(0),
                        cost_covered_success: row
                            .get::<_, Option<i64>>("cost_covered_success")?
                            .unwrap_or(0),
                        total_cost_usd_femto: row
                            .get::<_, Option<i64>>("total_cost_usd_femto")?
                            .unwrap_or(0),
                    };

                    Ok((cli_key, provider_id, provider_name, agg))
                })
                .map_err(|e| format!("DB_ERROR: failed to run provider leaderboard query: {e}"))?;

            let mut stmt_fallback_name = conn
                .prepare(
                    r#"
SELECT attempts_json
FROM request_logs r
WHERE r.excluded_from_stats = 0
AND r.final_provider_id = ?1
AND r.cli_key = ?2
AND (?3 IS NULL OR r.created_at >= ?3)
AND (?4 IS NULL OR r.created_at < ?4)
LIMIT 1
"#,
                )
                .map_err(|e| {
                    format!("DB_ERROR: failed to prepare provider name fallback query: {e}")
                })?;

            let mut items = Vec::new();
            for row in rows {
                items.push(row.map_err(|e| {
                    format!("DB_ERROR: failed to read provider leaderboard row: {e}")
                })?);
            }

            let mut out = Vec::new();
            for (cli_key, provider_id, provider_name_db, agg) in items {
                let mut provider_name = provider_name_db
                    .as_deref()
                    .map(str::trim)
                    .filter(|v| !v.is_empty() && *v != "Unknown")
                    .map(str::to_string);

                if provider_name.is_none() {
                    let attempts_json: Option<String> = stmt_fallback_name
                        .query_row(
                            params![provider_id, cli_key.as_str(), start_ts, end_ts],
                            |row| row.get(0),
                        )
                        .optional()
                        .map_err(|e| {
                            format!("DB_ERROR: failed to query provider name fallback: {e}")
                        })?;

                    if let Some(attempts_json) = attempts_json {
                        let extracted = extract_final_provider(&cli_key, &attempts_json);
                        let extracted_name = extracted.provider_name.trim();
                        if !extracted_name.is_empty() && extracted_name != "Unknown" {
                            provider_name = Some(extracted_name.to_string());
                        }
                    }
                }

                let Some(provider_name) = provider_name else {
                    continue;
                };

                let provider_key = ProviderKey {
                    cli_key: cli_key.clone(),
                    provider_id,
                    provider_name: provider_name.clone(),
                };
                if !has_valid_provider_key(&provider_key) {
                    continue;
                }

                out.push(agg.into_leaderboard_row(
                    format!("{}:{}", cli_key, provider_id),
                    format!("{}/{}", cli_key, provider_name),
                ));
            }

            out
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
    db: &db::Db,
    scope: &str,
    period: &str,
    start_ts: Option<i64>,
    end_ts: Option<i64>,
    cli_key: Option<&str>,
    limit: usize,
) -> Result<Vec<UsageLeaderboardRow>, String> {
    let conn = db.open_connection()?;
    let scope = parse_scope_v2(scope)?;
    let period = parse_period_v2(period)?;
    let (start_ts, end_ts) = compute_bounds_v2(&conn, period, start_ts, end_ts)?;
    let cli_key = normalize_cli_filter(cli_key)?;
    leaderboard_v2_with_conn(&conn, scope, start_ts, end_ts, cli_key, limit)
}
