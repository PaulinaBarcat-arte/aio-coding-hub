use super::SseTextAccumulator;

#[test]
fn sse_signature_delta_is_accumulated() {
    let mut acc = SseTextAccumulator::default();
    let sse = concat!(
        "event: message\n",
        "data: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"thinking\",\"thinking\":\"THINK1\",\"signature\":\"\"}}\n",
        "\n",
        "event: message\n",
        "data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"thinking_delta\",\"thinking\":\"THINK2\"}}\n",
        "\n",
        "event: message\n",
        "data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"signature_delta\",\"signature\":\"SIG_PART_1\"}}\n",
        "\n",
        "event: message\n",
        "data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"signature_delta\",\"signature\":\"SIG_PART_2\"}}\n",
        "\n",
        "event: message\n",
        "data: {\"type\":\"content_block_stop\",\"index\":0}\n",
        "\n",
    );

    acc.ingest_chunk(sse.as_bytes());
    acc.finalize();

    assert!(acc.thinking_block_seen);
    assert_eq!(acc.thinking_full, "THINK1THINK2");
    assert_eq!(acc.signature_full, "SIG_PART_1SIG_PART_2");
    assert!(acc.signature_from_delta);
    assert_eq!(acc.signature_chars, "SIG_PART_1SIG_PART_2".chars().count());
}

#[test]
fn sse_error_event_is_detected() {
    let mut acc = SseTextAccumulator::default();
    let sse = concat!(
        "event: error\n",
        "data: {\"error\":\"Claude API error\",\"status\":400,\"details\":\"{\\\"type\\\":\\\"error\\\",\\\"error\\\":{\\\"type\\\":\\\"invalid_request_error\\\",\\\"message\\\":\\\"This model does not support the effort parameter.\\\"},\\\"request_id\\\":\\\"req_123\\\"}\"}\n",
        "\n",
    );

    acc.ingest_chunk(sse.as_bytes());
    acc.finalize();

    assert!(acc.error_event_seen);
    assert_eq!(acc.error_status, Some(400));
    assert!(acc.error_message.contains("invalid_request_error"));
    assert!(acc
        .error_message
        .contains("This model does not support the effort parameter."));
    assert!(acc.error_message.contains("request_id=req_123"));
}
