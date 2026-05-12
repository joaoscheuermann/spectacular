use super::AgentConfig;
use crate::error::AgentError;
use spectacular_llms::{ProviderCapabilities, ToolManifest};

/// Builds the effective system prompt, including tool summaries when tools are enabled.
pub(super) fn effective_system_prompt(
    base_prompt: &str,
    tool_manifests: &[ToolManifest],
) -> String {
    if tool_manifests.is_empty() {
        return base_prompt.to_owned();
    }

    let tool_summary = format_tool_summary(tool_manifests);
    if base_prompt.trim().is_empty() {
        return tool_summary;
    }

    format!("{base_prompt}\n\n{tool_summary}")
}

/// Formats registered tools into provider-visible system prompt instructions.
fn format_tool_summary(tool_manifests: &[ToolManifest]) -> String {
    let tools = tool_manifests
        .iter()
        .map(|manifest| format!("* {} - {}", manifest.name, manifest.description))
        .collect::<Vec<_>>()
        .join("\n");

    format!("You have access to the following tools:\n{tools}")
}

/// Verifies that the selected provider can satisfy the configured run requirements.
pub(super) fn validate_provider_capabilities(
    capabilities: ProviderCapabilities,
    config: &AgentConfig,
    needs_tools: bool,
) -> Result<(), AgentError> {
    if !capabilities.streaming {
        return Err(AgentError::CapabilityMismatch {
            capability: "streaming",
        });
    }

    if config.require_usage_metadata && !capabilities.usage_metadata {
        return Err(AgentError::CapabilityMismatch {
            capability: "usage_metadata",
        });
    }

    if (config.include_reasoning || config.reasoning_effort.is_some()) && !capabilities.reasoning {
        return Err(AgentError::CapabilityMismatch {
            capability: "reasoning",
        });
    }

    if config.require_reasoning_metadata && !capabilities.reasoning_metadata {
        return Err(AgentError::CapabilityMismatch {
            capability: "reasoning_metadata",
        });
    }

    if needs_tools && !capabilities.tool_calls {
        return Err(AgentError::CapabilityMismatch {
            capability: "tool_calls",
        });
    }

    if config.output_schema.is_some() && !capabilities.structured_output {
        return Err(AgentError::CapabilityMismatch {
            capability: "structured_output",
        });
    }

    Ok(())
}
