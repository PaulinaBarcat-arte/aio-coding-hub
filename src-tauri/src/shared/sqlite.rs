//! Usage: Small helpers for SQLite data shapes.

pub(crate) fn enabled_to_int(enabled: bool) -> i64 {
    if enabled {
        1
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enabled_to_int_maps_bool() {
        assert_eq!(enabled_to_int(true), 1);
        assert_eq!(enabled_to_int(false), 0);
    }
}
