use super::style::{dim_style, error_style, paint, warning_style};
use spectacular_agent::ContextTokenUsage;

const WARNING_CONTEXT_RATIO_PERCENT: u64 = 80;
const CRITICAL_CONTEXT_RATIO_PERCENT: u64 = 90;

/// Formats context usage as a compact footer token segment with pressure styling.
pub(super) fn format_context_token_usage(usage: ContextTokenUsage) -> String {
    let text = match usage.context_window_tokens {
        Some(context_window_tokens) => format!(
            "{}/{} tks",
            compact_token_count(usage.input_tokens),
            compact_token_count(context_window_tokens)
        ),
        None => format!("{} tks", compact_token_count(usage.input_tokens)),
    };

    paint(context_pressure_style(usage), text)
}

/// Classifies context pressure and returns the terminal style for the usage segment.
pub(super) fn context_pressure_style(usage: ContextTokenUsage) -> anstyle::Style {
    let Some(context_window_tokens) = usage.context_window_tokens else {
        return dim_style();
    };
    if context_window_tokens == 0 {
        return dim_style();
    }

    let ratio_percent = usage.input_tokens.saturating_mul(100) / context_window_tokens;
    if ratio_percent >= CRITICAL_CONTEXT_RATIO_PERCENT {
        return error_style();
    }
    if ratio_percent >= WARNING_CONTEXT_RATIO_PERCENT {
        return warning_style();
    }

    dim_style()
}

/// Formats token counts compactly for footer display.
fn compact_token_count(tokens: u64) -> String {
    if tokens < 1_000 {
        return tokens.to_string();
    }

    format!("{}k", tokens / 1_000)
}
