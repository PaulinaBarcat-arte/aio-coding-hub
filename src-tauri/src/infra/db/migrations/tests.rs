use super::*;

#[test]
fn migrate_v25_to_v26_backfills_claude_models_json_from_legacy_mapping() {
    let mut conn = Connection::open_in_memory().expect("open in-memory sqlite");

    conn.execute_batch(
        r#"
CREATE TABLE providers (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  cli_key TEXT NOT NULL,
  name TEXT NOT NULL,
  base_url TEXT NOT NULL,
  base_urls_json TEXT NOT NULL DEFAULT '[]',
  base_url_mode TEXT NOT NULL DEFAULT 'order',
  api_key_plaintext TEXT NOT NULL,
  enabled INTEGER NOT NULL DEFAULT 1,
  priority INTEGER NOT NULL DEFAULT 100,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  sort_order INTEGER NOT NULL DEFAULT 0,
  cost_multiplier REAL NOT NULL DEFAULT 1.0,
  supported_models_json TEXT NOT NULL DEFAULT '{}',
  model_mapping_json TEXT NOT NULL DEFAULT '{}',
  UNIQUE(cli_key, name)
);
"#,
    )
    .expect("create providers table");

    let legacy_mapping = serde_json::json!({
        "*": "glm-4-plus",
        "claude-*sonnet*": "glm-4-plus-sonnet",
        "claude-*haiku*": "glm-4-plus-haiku",
        "claude-*thinking*": "glm-4-plus-thinking"
    })
    .to_string();

    conn.execute(
        r#"
INSERT INTO providers(
  cli_key,
  name,
  base_url,
  base_urls_json,
  base_url_mode,
  api_key_plaintext,
  enabled,
  priority,
  created_at,
  updated_at,
  sort_order,
  cost_multiplier,
  supported_models_json,
  model_mapping_json
) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 1, 100, 1, 1, 0, 1.0, '{}', ?7)
"#,
        rusqlite::params![
            "claude",
            "legacy",
            "https://example.com",
            "[]",
            "order",
            "sk-test",
            legacy_mapping
        ],
    )
    .expect("insert legacy provider");

    v25_to_v26::migrate_v25_to_v26(&mut conn).expect("migrate v25->v26");

    let claude_models_json: String = conn
        .query_row(
            "SELECT claude_models_json FROM providers WHERE name = 'legacy'",
            [],
            |row| row.get(0),
        )
        .expect("read claude_models_json");

    let value: serde_json::Value =
        serde_json::from_str(&claude_models_json).expect("claude_models_json valid json");

    assert_eq!(value["main_model"], "glm-4-plus");
    assert_eq!(value["sonnet_model"], "glm-4-plus-sonnet");
    assert_eq!(value["haiku_model"], "glm-4-plus-haiku");
    assert_eq!(value["reasoning_model"], "glm-4-plus-thinking");

    let supported_models_json: String = conn
        .query_row(
            "SELECT supported_models_json FROM providers WHERE name = 'legacy'",
            [],
            |row| row.get(0),
        )
        .expect("read supported_models_json");
    assert_eq!(supported_models_json.trim(), "{}");

    let model_mapping_json: String = conn
        .query_row(
            "SELECT model_mapping_json FROM providers WHERE name = 'legacy'",
            [],
            |row| row.get(0),
        )
        .expect("read model_mapping_json");
    assert_eq!(model_mapping_json.trim(), "{}");
}

#[test]
fn migrate_v27_to_v28_drops_provider_mode_and_deletes_official_providers() {
    let mut conn = Connection::open_in_memory().expect("open in-memory sqlite");
    conn.execute_batch("PRAGMA foreign_keys = ON;")
        .expect("enable foreign_keys");

    conn.execute_batch(
        r#"
CREATE TABLE providers (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  cli_key TEXT NOT NULL,
  name TEXT NOT NULL,
  base_url TEXT NOT NULL,
  base_urls_json TEXT NOT NULL DEFAULT '[]',
  base_url_mode TEXT NOT NULL DEFAULT 'order',
  claude_models_json TEXT NOT NULL DEFAULT '{}',
  api_key_plaintext TEXT NOT NULL,
  enabled INTEGER NOT NULL DEFAULT 1,
  priority INTEGER NOT NULL DEFAULT 100,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  sort_order INTEGER NOT NULL DEFAULT 0,
  cost_multiplier REAL NOT NULL DEFAULT 1.0,
  supported_models_json TEXT NOT NULL DEFAULT '{}',
  model_mapping_json TEXT NOT NULL DEFAULT '{}',
  provider_mode TEXT NOT NULL DEFAULT 'relay',
  UNIQUE(cli_key, name)
);

CREATE TABLE provider_circuit_breakers (
  provider_id INTEGER PRIMARY KEY,
  state TEXT NOT NULL,
  failure_count INTEGER NOT NULL DEFAULT 0,
  open_until INTEGER,
  updated_at INTEGER NOT NULL,
  FOREIGN KEY(provider_id) REFERENCES providers(id) ON DELETE CASCADE
);

CREATE TABLE sort_modes (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  name TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  UNIQUE(name)
);

CREATE TABLE sort_mode_providers (
  mode_id INTEGER NOT NULL,
  cli_key TEXT NOT NULL,
  provider_id INTEGER NOT NULL,
  sort_order INTEGER NOT NULL,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  PRIMARY KEY(mode_id, cli_key, provider_id),
  FOREIGN KEY(mode_id) REFERENCES sort_modes(id) ON DELETE CASCADE,
  FOREIGN KEY(provider_id) REFERENCES providers(id) ON DELETE CASCADE
);

CREATE TABLE claude_model_validation_runs (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  provider_id INTEGER NOT NULL,
  created_at INTEGER NOT NULL,
  request_json TEXT NOT NULL,
  result_json TEXT NOT NULL,
  FOREIGN KEY(provider_id) REFERENCES providers(id) ON DELETE CASCADE
);
"#,
    )
    .expect("create v27 schema");

    conn.execute(
        r#"
INSERT INTO providers(
  id,
  cli_key,
  name,
  base_url,
  base_urls_json,
  base_url_mode,
  claude_models_json,
  api_key_plaintext,
  enabled,
  priority,
  created_at,
  updated_at,
  sort_order,
  cost_multiplier,
  supported_models_json,
  model_mapping_json,
  provider_mode
) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)
"#,
        rusqlite::params![
            1i64,
            "codex",
            "relay",
            "https://relay.example.com/v1",
            "[\"https://relay.example.com/v1\"]",
            "order",
            "{}",
            "sk-relay",
            1i64,
            100i64,
            1i64,
            1i64,
            0i64,
            1.0f64,
            "{}",
            "{}",
            "relay",
        ],
    )
    .expect("insert relay provider");

    conn.execute(
        r#"
INSERT INTO providers(
  id,
  cli_key,
  name,
  base_url,
  base_urls_json,
  base_url_mode,
  claude_models_json,
  api_key_plaintext,
  enabled,
  priority,
  created_at,
  updated_at,
  sort_order,
  cost_multiplier,
  supported_models_json,
  model_mapping_json,
  provider_mode
) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)
"#,
        rusqlite::params![
            2i64,
            "codex",
            "official",
            "https://api.openai.com/v1",
            "[\"https://api.openai.com/v1\"]",
            "order",
            "{}",
            "",
            1i64,
            100i64,
            1i64,
            1i64,
            1i64,
            1.0f64,
            "{}",
            "{}",
            "official",
        ],
    )
    .expect("insert official provider");

    conn.execute(
            "INSERT INTO provider_circuit_breakers(provider_id, state, failure_count, open_until, updated_at) VALUES (?1, 'CLOSED', 0, NULL, 1)",
            rusqlite::params![1i64],
        )
        .expect("insert relay breaker");
    conn.execute(
            "INSERT INTO provider_circuit_breakers(provider_id, state, failure_count, open_until, updated_at) VALUES (?1, 'CLOSED', 0, NULL, 1)",
            rusqlite::params![2i64],
        )
        .expect("insert official breaker");

    conn.execute(
        "INSERT INTO sort_modes(id, name, created_at, updated_at) VALUES (1, 'mode', 1, 1)",
        [],
    )
    .expect("insert sort mode");
    conn.execute(
            "INSERT INTO sort_mode_providers(mode_id, cli_key, provider_id, sort_order, created_at, updated_at) VALUES (1, 'codex', 1, 0, 1, 1)",
            [],
        )
        .expect("insert relay sort_mode_provider");
    conn.execute(
            "INSERT INTO sort_mode_providers(mode_id, cli_key, provider_id, sort_order, created_at, updated_at) VALUES (1, 'codex', 2, 1, 1, 1)",
            [],
        )
        .expect("insert official sort_mode_provider");

    conn.execute(
            "INSERT INTO claude_model_validation_runs(id, provider_id, created_at, request_json, result_json) VALUES (1, 1, 1, '{}', '{}')",
            [],
        )
        .expect("insert relay validation run");
    conn.execute(
            "INSERT INTO claude_model_validation_runs(id, provider_id, created_at, request_json, result_json) VALUES (2, 2, 1, '{}', '{}')",
            [],
        )
        .expect("insert official validation run");

    v27_to_v28::migrate_v27_to_v28(&mut conn).expect("migrate v27->v28");

    let user_version: i64 = conn
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .expect("read user_version");
    assert_eq!(user_version, 28);

    let provider_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM providers", [], |row| row.get(0))
        .expect("count providers");
    assert_eq!(provider_count, 1);

    let breaker_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM provider_circuit_breakers",
            [],
            |row| row.get(0),
        )
        .expect("count breakers");
    assert_eq!(breaker_count, 1);

    let sort_mode_provider_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM sort_mode_providers", [], |row| {
            row.get(0)
        })
        .expect("count sort_mode_providers");
    assert_eq!(sort_mode_provider_count, 1);

    let validation_run_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM claude_model_validation_runs",
            [],
            |row| row.get(0),
        )
        .expect("count validation runs");
    assert_eq!(validation_run_count, 1);

    let remaining_name: String = conn
        .query_row("SELECT name FROM providers WHERE id = 1", [], |row| {
            row.get(0)
        })
        .expect("read remaining provider name");
    assert_eq!(remaining_name, "relay");

    let mut has_provider_mode = false;
    {
        let mut stmt = conn
            .prepare("PRAGMA table_info(providers)")
            .expect("prepare providers table_info query");
        let mut rows = stmt.query([]).expect("query providers table_info");
        while let Some(row) = rows.next().expect("read table_info row") {
            let name: String = row.get(1).expect("read column name");
            if name == "provider_mode" {
                has_provider_mode = true;
                break;
            }
        }
    }
    assert!(!has_provider_mode);
}
