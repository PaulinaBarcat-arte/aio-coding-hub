//! Usage: Small shared string helpers.

pub(crate) fn normalize_name(name: &str) -> String {
    name.trim().to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_name_trims_and_lowercases() {
        assert_eq!(normalize_name("  AbC  "), "abc");
    }
}
