use crate::chat::commands::ChatCompletionContext;
use crate::chat::ChatError;
use spectacular_config::{ReasoningLevel, TaskModelSlot};
use spectacular_llms::OPENAI_PROVIDER_ID;

/// Returns enabled provider backend ids from the provider registry.
pub(super) fn enabled_provider_type_values(
    ctx: &ChatCompletionContext<'_>,
) -> Result<Vec<String>, ChatError> {
    Ok(ctx.enabled_provider_type_ids())
}

/// Returns no suggestions for free-form or secret command fields.
pub(super) fn no_values(_: &ChatCompletionContext<'_>) -> Result<Vec<String>, ChatError> {
    Ok(Vec::new())
}

/// Returns configured provider names from persisted chat configuration.
pub(super) fn configured_provider_values(
    ctx: &ChatCompletionContext<'_>,
) -> Result<Vec<String>, ChatError> {
    ctx.configured_provider_names()
}

/// Returns the literal `true` value used by destructive-action confirmations.
pub(super) fn confirm_true_values(_: &ChatCompletionContext<'_>) -> Result<Vec<String>, ChatError> {
    Ok(vec!["true".to_owned()])
}

/// Returns the OpenAI provider id used by the provider browser-auth flow.
pub(super) fn openai_provider_values(
    _: &ChatCompletionContext<'_>,
) -> Result<Vec<String>, ChatError> {
    Ok(vec![OPENAI_PROVIDER_ID.to_owned()])
}

/// Returns cached model ids scoped by typed provider, inferred edited model, or all providers.
pub(super) fn cached_model_id_values(
    ctx: &ChatCompletionContext<'_>,
) -> Result<Vec<String>, ChatError> {
    let provider = typed_or_inferred_provider(ctx)?;
    ctx.cached_model_ids(provider.as_deref())
}

/// Returns the provider typed in args or inferred from the edited model name.
fn typed_or_inferred_provider(
    ctx: &ChatCompletionContext<'_>,
) -> Result<Option<String>, ChatError> {
    if let Some(provider) = ctx.args.get("provider") {
        return Ok(Some(provider.to_owned()));
    }

    if ctx.subcommand != "edit" {
        return Ok(None);
    }

    let Some(model_name) = ctx.args.get("name") else {
        return Ok(None);
    };

    ctx.saved_model_provider(model_name)
}

/// Returns the supported reasoning-level completion values from the canonical config enum.
pub(super) fn reasoning_values(_: &ChatCompletionContext<'_>) -> Result<Vec<String>, ChatError> {
    Ok(ReasoningLevel::ALL
        .into_iter()
        .map(|value| value.as_str().to_owned())
        .collect())
}

/// Returns saved model aliases from persisted chat configuration.
pub(super) fn saved_model_values(
    ctx: &ChatCompletionContext<'_>,
) -> Result<Vec<String>, ChatError> {
    ctx.saved_model_names()
}

/// Returns the supported task-slot completion values from the canonical config enum.
pub(super) fn task_values(_: &ChatCompletionContext<'_>) -> Result<Vec<String>, ChatError> {
    Ok(TaskModelSlot::ALL
        .into_iter()
        .map(|slot| slot.as_str().to_owned())
        .collect())
}
