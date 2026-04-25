use spectacular_llms::enabled_provider_capability_report;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let report = enabled_provider_capability_report()?;
    let metadata = report.metadata;
    let capabilities = report.capabilities;

    println!("Provider: {} ({})", metadata.display_name(), metadata.id());
    println!("Enabled: {}", metadata.is_enabled());
    println!("Agent capabilities:");
    println!("  streaming: {}", capabilities.streaming);
    println!("  tools: {}", capabilities.tool_calls);
    println!("  structured output: {}", capabilities.structured_output);
    println!("  cancellation: {}", capabilities.cancellation);
    println!("  usage metadata: {}", capabilities.usage_metadata);
    println!("  reasoning metadata: {}", capabilities.reasoning_metadata);
    println!("  reasoning: {}", capabilities.reasoning);
    println!(
        "  context max messages: {}",
        optional_usize(capabilities.context_limits.max_messages)
    );
    println!(
        "  context max chars: {}",
        optional_usize(capabilities.context_limits.max_chars)
    );

    Ok(())
}

fn optional_usize(value: Option<usize>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "unbounded".to_owned())
}
