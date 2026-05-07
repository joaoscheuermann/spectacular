fn format_config_report(config: &SpectacularConfig) -> String {
    let mut lines = Vec::new();

    lines.push(paint(title_style(), "Spectacular config"));
    lines.push(String::new());
    lines.push(paint(section_style(), "Providers"));

    if config.providers.is_empty() {
        lines.push(format!("  {}", paint(missing_style(), "None")));
    } else {
        for (name, provider) in &config.providers {
            lines.push(format!(
                "  {} {} {} {}",
                paint(provider_style(), name),
                paint(label_style(), "type:"),
                paint(provider_style(), &provider.provider_type),
                paint(secret_style(), mask_api_key(&provider.apikey))
            ));
        }
    }

    lines.push(String::new());
    lines.push(paint(section_style(), "Models"));
    if config.models.is_empty() {
        lines.push(format!("  {}", paint(missing_style(), "None")));
    } else {
        for (name, model) in &config.models {
            let provider_state = if config.providers.contains_key(&model.provider) {
                String::new()
            } else {
                format!(" {}", paint(missing_style(), "(provider missing)"))
            };
            lines.push(format!(
                "  {} {} {} {} {} {}{}",
                paint(model_style(), name),
                paint(label_style(), "provider:"),
                paint(provider_style(), &model.provider),
                paint(label_style(), "id:"),
                paint(model_style(), &model.model),
                paint_reasoning(model.reasoning, ""),
                provider_state
            ));
        }
    }

    lines.push(String::new());
    lines.push(paint(section_style(), "Tasks"));
    for slot in TaskModelSlot::ALL {
        append_task_report(&mut lines, config, slot);
    }

    lines.join("\n")
}

fn append_task_report(lines: &mut Vec<String>, config: &SpectacularConfig, slot: TaskModelSlot) {
    let Some(model_key) = config
        .tasks
        .get(slot)
        .filter(|value| !value.trim().is_empty())
    else {
        lines.push(format!(
            "  {} {}",
            paint(task_style(), format!("{slot}:")),
            paint(missing_style(), "not configured")
        ));
        return;
    };
    let state = if config.models.contains_key(model_key) {
        String::new()
    } else {
        format!(" {}", paint(missing_style(), "(model not found)"))
    };

    lines.push(format!(
        "  {} {}{}",
        paint(task_style(), format!("{slot}:")),
        paint(model_style(), model_key),
        state
    ));
}

fn format_provider_added_output(name: &str, provider_type: &str) -> String {
    format!(
        "{} {}\n  {} {}\n  {} {}",
        paint(success_style(), "[saved]"),
        paint(title_style(), "Provider added"),
        paint(label_style(), "Name:"),
        paint(provider_style(), name),
        paint(label_style(), "Type:"),
        paint(provider_style(), provider_type)
    )
}

fn format_provider_removed_output(name: &str) -> String {
    format!(
        "{} {}\n  {} {}",
        paint(success_style(), "[removed]"),
        paint(title_style(), "Provider removed"),
        paint(label_style(), "Name:"),
        paint(provider_style(), name)
    )
}

fn format_model_saved_output(action: &str, key: &str) -> String {
    format!(
        "{} {}\n  {} {}",
        paint(success_style(), "[saved]"),
        paint(title_style(), format!("Model {action}")),
        paint(label_style(), "Name:"),
        paint(model_style(), key)
    )
}

fn format_model_remove_confirmation_output(name: &str, references: &[TaskModelSlot]) -> String {
    if references.is_empty() {
        return format_confirmation_required_output(
            "Model removal requires confirm:true. No tasks currently reference this model.",
        );
    }

    format_confirmation_required_output(&format!(
        "Model `{name}` is used by tasks: {}. Re-run with confirm:true to delete it and leave those task references invalid.",
        references
            .iter()
            .map(|slot| slot.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    ))
}

fn format_model_removed_output(name: &str, references: &[TaskModelSlot]) -> String {
    let warning = if references.is_empty() {
        String::new()
    } else {
        format!(
            "\n  {} {}",
            paint(label_style(), "Invalid tasks:"),
            paint(
                missing_style(),
                references
                    .iter()
                    .map(|slot| slot.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        )
    };

    format!(
        "{} {}\n  {} {}{}",
        paint(success_style(), "[removed]"),
        paint(title_style(), "Model removed"),
        paint(label_style(), "Name:"),
        paint(model_style(), name),
        warning
    )
}

fn format_task_saved_output(slot: TaskModelSlot, model: &str) -> String {
    format!(
        "{} {}\n  {} {}\n  {} {}",
        paint(success_style(), "[saved]"),
        paint(title_style(), "Task model assigned"),
        paint(label_style(), "Task:"),
        paint(task_style(), slot.as_str()),
        paint(label_style(), "Model:"),
        paint(model_style(), model)
    )
}

fn format_confirmation_required_output(message: &str) -> String {
    format!(
        "{} {}",
        paint(missing_style(), "[confirmation required]"),
        message
    )
}

fn paint(style: Style, value: impl AsRef<str>) -> String {
    terminal_style::paint(style, value)
}

fn paint_reasoning(reasoning: ReasoningLevel, suffix: &str) -> String {
    paint(reasoning_style(reasoning), format!("{reasoning}{suffix}"))
}

fn title_style() -> Style {
    terminal_style::title_style()
}

fn section_style() -> Style {
    terminal_style::text_style().bold()
}

fn label_style() -> Style {
    terminal_style::dim_style()
}

fn success_style() -> Style {
    terminal_style::success_style()
}

fn provider_style() -> Style {
    terminal_style::provider_style()
}

fn task_style() -> Style {
    terminal_style::task_style()
}

fn model_style() -> Style {
    terminal_style::model_style()
}

fn secret_style() -> Style {
    terminal_style::secret_style()
}

fn missing_style() -> Style {
    terminal_style::warning_style()
}

fn reasoning_style(reasoning: ReasoningLevel) -> Style {
    match reasoning {
        ReasoningLevel::None => terminal_style::dim_style(),
        ReasoningLevel::Minimal | ReasoningLevel::Low => terminal_style::low_reasoning_style(),
        ReasoningLevel::Medium => terminal_style::warning_style(),
        ReasoningLevel::High | ReasoningLevel::Xhigh => terminal_style::error_style(),
    }
}
