use super::*;

/// Verifies context usage keeps only total input tokens and model window.
#[test]
fn context_token_usage_uses_total_input_and_model_window() {
    let diagnostics = ContextDiagnostics {
        total_input_tokens: 123,
        usable_input_tokens: Some(100),
        active_compaction_threshold: None,
        soft_compaction_threshold: None,
        max_output_tokens: 10,
        reasoning_reserve_tokens: 0,
        safety_margin_tokens: 0,
        message_count: 1,
        section_usage: Vec::new(),
        soft_compaction_would_trigger: false,
        compaction_would_trigger: false,
    };
    let policy = ContextPolicy {
        model_context_window_tokens: Some(240_000),
        ..ContextPolicy::default()
    };

    assert_eq!(
        context_token_usage(&diagnostics, &policy),
        ContextTokenUsage {
            input_tokens: 123,
            context_window_tokens: Some(240_000),
        }
    );
}
