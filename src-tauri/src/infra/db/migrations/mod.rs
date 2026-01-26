//! Usage: SQLite schema migrations (user_version + incremental upgrades).

mod v0_to_v1;
mod v10_to_v11;
mod v11_to_v12;
mod v12_to_v13;
mod v13_to_v14;
mod v14_to_v15;
mod v15_to_v16;
mod v16_to_v17;
mod v17_to_v18;
mod v18_to_v19;
mod v19_to_v20;
mod v1_to_v2;
mod v20_to_v21;
mod v21_to_v22;
mod v22_to_v23;
mod v23_to_v24;
mod v24_to_v25;
mod v25_to_v26;
mod v26_to_v27;
mod v27_to_v28;
mod v2_to_v3;
mod v3_to_v4;
mod v4_to_v5;
mod v5_to_v6;
mod v6_to_v7;
mod v7_to_v8;
mod v8_to_v9;
mod v9_to_v10;

use rusqlite::Connection;

const LATEST_SCHEMA_VERSION: i64 = 28;

pub(super) fn apply_migrations(conn: &mut Connection) -> Result<(), String> {
    let mut user_version = read_user_version(conn)?;

    if user_version < 0 {
        return Err(format!(
            "unsupported sqlite schema version: user_version={user_version} (expected 0..={LATEST_SCHEMA_VERSION})"
        ));
    }

    if user_version > LATEST_SCHEMA_VERSION {
        return Err(format!(
            "unsupported sqlite schema version: user_version={user_version} (expected 0..={LATEST_SCHEMA_VERSION})"
        ));
    }

    while user_version < LATEST_SCHEMA_VERSION {
        match user_version {
            0 => v0_to_v1::migrate_v0_to_v1(conn)?,
            1 => v1_to_v2::migrate_v1_to_v2(conn)?,
            2 => v2_to_v3::migrate_v2_to_v3(conn)?,
            3 => v3_to_v4::migrate_v3_to_v4(conn)?,
            4 => v4_to_v5::migrate_v4_to_v5(conn)?,
            5 => v5_to_v6::migrate_v5_to_v6(conn)?,
            6 => v6_to_v7::migrate_v6_to_v7(conn)?,
            7 => v7_to_v8::migrate_v7_to_v8(conn)?,
            8 => v8_to_v9::migrate_v8_to_v9(conn)?,
            9 => v9_to_v10::migrate_v9_to_v10(conn)?,
            10 => v10_to_v11::migrate_v10_to_v11(conn)?,
            11 => v11_to_v12::migrate_v11_to_v12(conn)?,
            12 => v12_to_v13::migrate_v12_to_v13(conn)?,
            13 => v13_to_v14::migrate_v13_to_v14(conn)?,
            14 => v14_to_v15::migrate_v14_to_v15(conn)?,
            15 => v15_to_v16::migrate_v15_to_v16(conn)?,
            16 => v16_to_v17::migrate_v16_to_v17(conn)?,
            17 => v17_to_v18::migrate_v17_to_v18(conn)?,
            18 => v18_to_v19::migrate_v18_to_v19(conn)?,
            19 => v19_to_v20::migrate_v19_to_v20(conn)?,
            20 => v20_to_v21::migrate_v20_to_v21(conn)?,
            21 => v21_to_v22::migrate_v21_to_v22(conn)?,
            22 => v22_to_v23::migrate_v22_to_v23(conn)?,
            23 => v23_to_v24::migrate_v23_to_v24(conn)?,
            24 => v24_to_v25::migrate_v24_to_v25(conn)?,
            25 => v25_to_v26::migrate_v25_to_v26(conn)?,
            26 => v26_to_v27::migrate_v26_to_v27(conn)?,
            27 => v27_to_v28::migrate_v27_to_v28(conn)?,
            v => {
                return Err(format!(
                    "unsupported sqlite schema version: user_version={v} (expected 0..={LATEST_SCHEMA_VERSION})"
                ))
            }
        }
        user_version = read_user_version(conn)?;
    }

    Ok(())
}

fn read_user_version(conn: &Connection) -> Result<i64, String> {
    conn.pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(|e| format!("failed to read sqlite user_version: {e}"))
}

fn set_user_version(tx: &rusqlite::Transaction<'_>, version: i64) -> Result<(), String> {
    tx.pragma_update(None, "user_version", version)
        .map_err(|e| format!("failed to update sqlite user_version: {e}"))?;
    Ok(())
}

#[cfg(test)]
mod tests;
