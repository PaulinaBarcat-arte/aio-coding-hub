use super::*;

#[test]
fn wildcard_single_star_matches_prefix_suffix() {
    assert!(match_wildcard_single("a*b", "axxb"));
    assert!(match_wildcard_single("*b", "b"));
    assert!(match_wildcard_single("a*", "a"));
    assert!(!match_wildcard_single("a*b", "abx"));
    assert!(!match_wildcard_single("a*b*c", "abc"));
}

#[test]
fn resolves_exact_over_wildcard_over_prefix() {
    let aliases = ModelPriceAliasesV1 {
        version: 1,
        rules: vec![
            ModelPriceAliasRuleV1 {
                cli_key: "gemini".to_string(),
                match_type: ModelPriceAliasMatchTypeV1::Prefix,
                pattern: "gemini-3".to_string(),
                target_model: "gemini-3-any".to_string(),
                enabled: true,
            },
            ModelPriceAliasRuleV1 {
                cli_key: "gemini".to_string(),
                match_type: ModelPriceAliasMatchTypeV1::Wildcard,
                pattern: "gemini-3-*".to_string(),
                target_model: "gemini-3-wild".to_string(),
                enabled: true,
            },
            ModelPriceAliasRuleV1 {
                cli_key: "gemini".to_string(),
                match_type: ModelPriceAliasMatchTypeV1::Exact,
                pattern: "gemini-3-flash".to_string(),
                target_model: "gemini-3-flash-preview".to_string(),
                enabled: true,
            },
        ],
    };

    assert_eq!(
        aliases.resolve_target_model("gemini", "gemini-3-flash"),
        Some("gemini-3-flash-preview")
    );
    assert_eq!(
        aliases.resolve_target_model("gemini", "gemini-3-pro"),
        Some("gemini-3-wild")
    );
}

#[test]
fn resolves_longer_patterns_first_within_same_type() {
    let aliases = ModelPriceAliasesV1 {
        version: 1,
        rules: vec![
            ModelPriceAliasRuleV1 {
                cli_key: "claude".to_string(),
                match_type: ModelPriceAliasMatchTypeV1::Prefix,
                pattern: "claude-opus".to_string(),
                target_model: "a".to_string(),
                enabled: true,
            },
            ModelPriceAliasRuleV1 {
                cli_key: "claude".to_string(),
                match_type: ModelPriceAliasMatchTypeV1::Prefix,
                pattern: "claude-opus-4-5".to_string(),
                target_model: "b".to_string(),
                enabled: true,
            },
        ],
    };

    assert_eq!(
        aliases.resolve_target_model("claude", "claude-opus-4-5-thinking"),
        Some("b")
    );
}
