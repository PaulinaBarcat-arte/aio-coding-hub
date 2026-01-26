use super::*;

#[test]
fn parses_decimal_with_exponent_to_femto() {
    let femto = parse_decimal_to_femto("1.5e-6").expect("parse");
    // 0.0000015 * 1e15 = 1.5e9
    assert_eq!(femto, 1_500_000_000);
}

#[test]
fn calculates_basic_cost() {
    let usage = CostUsage {
        input_tokens: 10,
        output_tokens: 5,
        ..Default::default()
    };
    let price_json = r#"{"input_cost_per_token":0.01,"output_cost_per_token":0.02}"#;
    let cost = calculate_cost_usd_femto(&usage, price_json, 1.0, "codex", "gpt").expect("cost");

    let expected = (10i128 * 10_000_000_000_000i128) + (5i128 * 20_000_000_000_000i128);
    assert_eq!(cost as i128, expected);
}

#[test]
fn tiered_cost_with_separate_prices_applies_above_200k() {
    let usage = CostUsage {
        input_tokens: 200_001,
        ..Default::default()
    };
    let price_json = r#"{
      "input_cost_per_token": 0.01,
      "input_cost_per_token_above_200k_tokens": 0.02
    }"#;
    let cost =
        calculate_cost_usd_femto(&usage, price_json, 1.0, "gemini", "gemini-test").expect("cost");

    let base = 200_000i128 * 10_000_000_000_000i128;
    let premium = 20_000_000_000_000i128;
    assert_eq!(cost as i128, base + premium);
}

#[test]
fn tiered_cost_with_context_1m_multiplier_applies_for_claude_1m_model() {
    let usage = CostUsage {
        input_tokens: 200_001,
        output_tokens: 200_001,
        ..Default::default()
    };
    let price_json = r#"{
      "input_cost_per_token": 0.01,
      "output_cost_per_token": 0.02
    }"#;
    let cost =
        calculate_cost_usd_femto(&usage, price_json, 1.0, "claude", "claude-1m").expect("cost");

    let input_base = 200_000i128 * 10_000_000_000_000i128;
    let input_premium = 20_000_000_000_000i128; // 2x

    let output_base = 200_000i128 * 20_000_000_000_000i128;
    let output_premium = 30_000_000_000_000i128; // 1.5x

    assert_eq!(
        cost as i128,
        input_base + input_premium + output_base + output_premium
    );
}

#[test]
fn applies_provider_multiplier() {
    let usage = CostUsage {
        input_tokens: 10,
        ..Default::default()
    };
    let price_json = r#"{"input_cost_per_token":0.01}"#;
    let cost = calculate_cost_usd_femto(&usage, price_json, 1.5, "codex", "gpt").expect("cost");

    let base = 10i128 * 10_000_000_000_000i128;
    let expected = base.saturating_mul(1_500_000) / 1_000_000;
    assert_eq!(cost as i128, expected);
}

#[test]
fn calculates_cost_with_basellm_exponent_price_json() {
    let usage = CostUsage {
        input_tokens: 100,
        output_tokens: 20,
        cache_read_input_tokens: 50,
        cache_creation_5m_input_tokens: 10,
        cache_creation_1h_input_tokens: 5,
        ..Default::default()
    };

    let price_json = r#"{
      "cache_creation_input_token_cost":"3.75e-6",
      "cache_creation_input_token_cost_above_1hr":"3.75e-6",
      "cache_read_input_token_cost":"0.3e-6",
      "input_cost_per_token":"3e-6",
      "output_cost_per_token":"15e-6"
    }"#;

    let cost = calculate_cost_usd_femto(&usage, price_json, 1.0, "codex", "gpt").expect("cost");
    assert_eq!(cost, 521_250_000_000);
}

#[test]
fn codex_does_not_double_charge_cached_input_tokens() {
    let usage = CostUsage {
        input_tokens: 100,
        output_tokens: 10,
        cache_read_input_tokens: 80,
        ..Default::default()
    };

    let price_json = r#"{
      "input_cost_per_token": 0.01,
      "output_cost_per_token": 0.02,
      "cache_read_input_token_cost": 0.001
    }"#;

    let cost = calculate_cost_usd_femto(&usage, price_json, 1.0, "codex", "gpt-5.2").expect("cost");

    let input = 10_000_000_000_000i128;
    let output = 20_000_000_000_000i128;
    let cache_read = 1_000_000_000_000i128;

    let expected = (20i128 * input) + (10i128 * output) + (80i128 * cache_read);
    assert_eq!(cost as i128, expected);
}

#[test]
fn gemini_does_not_double_charge_cached_input_tokens() {
    let usage = CostUsage {
        input_tokens: 100,
        output_tokens: 10,
        cache_read_input_tokens: 80,
        ..Default::default()
    };

    let price_json = r#"{
      "input_cost_per_token": 0.01,
      "output_cost_per_token": 0.02,
      "cache_read_input_token_cost": 0.001
    }"#;

    let cost =
        calculate_cost_usd_femto(&usage, price_json, 1.0, "gemini", "gemini-test").expect("cost");

    let input = 10_000_000_000_000i128;
    let output = 20_000_000_000_000i128;
    let cache_read = 1_000_000_000_000i128;

    let expected = (20i128 * input) + (10i128 * output) + (80i128 * cache_read);
    assert_eq!(cost as i128, expected);
}

#[test]
fn claude_keeps_cache_read_additive_cost() {
    let usage = CostUsage {
        input_tokens: 100,
        cache_read_input_tokens: 80,
        ..Default::default()
    };

    let price_json = r#"{
      "input_cost_per_token": 0.01,
      "cache_read_input_token_cost": 0.001
    }"#;

    let cost =
        calculate_cost_usd_femto(&usage, price_json, 1.0, "claude", "claude-test").expect("cost");

    let input = 10_000_000_000_000i128;
    let cache_read = 1_000_000_000_000i128;

    let expected = (100i128 * input) + (80i128 * cache_read);
    assert_eq!(cost as i128, expected);
}
