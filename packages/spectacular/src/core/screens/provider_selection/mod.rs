use crate::core::keys::is_cancel_key;
use crate::core::navigation::{next_index, previous_index};
use crate::core::terminal::TerminalSession;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{List, ListItem, ListState, Paragraph, Wrap},
    Frame,
};
use std::error::Error;
use std::fmt::{self, Display};
use std::io;

const API_KEY_VISIBLE_PREFIX_CHARS: usize = 13;
const MIN_MASK_CHARS: usize = 8;

/// Provider row view model consumed by the provider selection screen.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderOption {
    id: String,
    display_name: String,
    enabled: bool,
    masked_api_key: Option<String>,
    active: bool,
}

impl ProviderOption {
    /// Builds a provider row while ensuring stored API keys are masked for display.
    pub fn new(
        id: impl Into<String>,
        display_name: impl Into<String>,
        enabled: bool,
        stored_api_key: Option<&str>,
        active: bool,
    ) -> Self {
        Self {
            id: id.into(),
            display_name: display_name.into(),
            enabled,
            masked_api_key: stored_api_key.and_then(mask_api_key),
            active,
        }
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn masked_api_key(&self) -> Option<&str> {
        self.masked_api_key.as_deref()
    }

    pub fn is_active(&self) -> bool {
        self.active
    }
}

/// Masks a stored API key before it is displayed in terminal output.
pub fn mask_api_key(api_key: &str) -> Option<String> {
    let trimmed = api_key.trim();

    if trimmed.is_empty() {
        return None;
    }

    let total_len = trimmed.chars().count();
    let visible_prefix_len = if total_len > API_KEY_VISIBLE_PREFIX_CHARS {
        API_KEY_VISIBLE_PREFIX_CHARS
    } else {
        total_len.saturating_sub(4).min(4)
    };
    let prefix: String = trimmed.chars().take(visible_prefix_len).collect();
    let prefix_len = prefix.chars().count();
    let hidden_len = trimmed
        .chars()
        .count()
        .saturating_sub(prefix_len)
        .max(MIN_MASK_CHARS);

    Some(format!("{prefix}{}", "*".repeat(hidden_len)))
}

/// Selects an enabled provider by id, returning a clear error for disabled rows.
pub fn select_provider_id(
    providers: &[ProviderOption],
    provider_id: &str,
) -> Result<String, ProviderSelectionError> {
    let provider = providers
        .iter()
        .find(|provider| provider.id == provider_id)
        .ok_or_else(|| ProviderSelectionError::UnknownProvider {
            provider_id: provider_id.to_owned(),
        })?;

    if !provider.enabled {
        return Err(ProviderSelectionError::DisabledProvider {
            provider_name: provider.display_name.clone(),
        });
    }

    Ok(provider.id.clone())
}

/// Runs the interactive Ratatui provider selection screen.
pub fn run_provider_selection_screen(
    providers: &[ProviderOption],
) -> Result<String, ProviderSelectionError> {
    if !providers.iter().any(|provider| provider.enabled) {
        return Err(ProviderSelectionError::NoEnabledProviders);
    }

    let mut session = TerminalSession::start().map_err(ProviderSelectionError::TerminalIo)?;
    let mut selected_index = initial_selected_index(providers);
    let mut status_message: Option<String> = None;

    loop {
        session.terminal.draw(|frame| {
            render_provider_selection(frame, providers, selected_index, status_message.as_deref());
        })?;

        let Event::Key(key) = event::read()? else {
            continue;
        };

        if key.kind != KeyEventKind::Press {
            continue;
        }

        if is_cancel_key(&key.code) {
            return Err(ProviderSelectionError::Cancelled);
        }

        match key.code {
            KeyCode::Up => {
                selected_index = previous_index(selected_index, providers.len());
                status_message = None;
            }
            KeyCode::Down => {
                selected_index = next_index(selected_index, providers.len());
                status_message = None;
            }
            KeyCode::Enter => match select_provider_id(providers, providers[selected_index].id()) {
                Ok(provider_id) => return Ok(provider_id),
                Err(error @ ProviderSelectionError::DisabledProvider { .. }) => {
                    status_message = Some(error.to_string());
                }
                Err(error) => return Err(error),
            },
            _ => {}
        }
    }
}

/// Renders the provider selection screen into the supplied Ratatui frame.
pub fn render_provider_selection(
    frame: &mut Frame<'_>,
    providers: &[ProviderOption],
    selected_index: usize,
    status_message: Option<&str>,
) {
    let areas = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(2),
            Constraint::Min(5),
            Constraint::Length(2),
        ])
        .split(frame.area());

    frame.render_widget(
        Paragraph::new("Spectacular Providers").style(Style::default().fg(Color::Blue)),
        areas[0],
    );

    frame.render_widget(
        Paragraph::new("Select the provider used by Spectacular to run LLM inferences.")
            .wrap(Wrap { trim: true }),
        areas[1],
    );

    let items = provider_list_items(providers);
    let list = List::new(items).highlight_style(Style::default().add_modifier(Modifier::REVERSED));
    let mut state = ListState::default();
    state.select(Some(selected_index.min(providers.len().saturating_sub(1))));

    frame.render_stateful_widget(list, areas[2], &mut state);

    frame.render_widget(
        Paragraph::new(
            status_message.unwrap_or("Use arrow keys to move, Enter to select, Esc to quit."),
        ),
        areas[3],
    );
}

/// Errors returned by provider selection UI and selection rules.
#[derive(Debug)]
pub enum ProviderSelectionError {
    Cancelled,
    DisabledProvider { provider_name: String },
    NoEnabledProviders,
    UnknownProvider { provider_id: String },
    TerminalIo(io::Error),
}

impl From<io::Error> for ProviderSelectionError {
    fn from(source: io::Error) -> Self {
        Self::TerminalIo(source)
    }
}

impl Display for ProviderSelectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProviderSelectionError::Cancelled => {
                formatter.write_str("Provider selection was cancelled.")
            }
            ProviderSelectionError::DisabledProvider { provider_name } => {
                write!(
                    formatter,
                    "Provider `{provider_name}` is disabled and cannot be selected."
                )
            }
            ProviderSelectionError::NoEnabledProviders => {
                formatter.write_str("No enabled providers are available.")
            }
            ProviderSelectionError::UnknownProvider { provider_id } => {
                write!(formatter, "Provider `{provider_id}` is not available.")
            }
            ProviderSelectionError::TerminalIo(source) => {
                write!(formatter, "Provider selection failed: {source}")
            }
        }
    }
}

impl Error for ProviderSelectionError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            ProviderSelectionError::TerminalIo(source) => Some(source),
            ProviderSelectionError::Cancelled
            | ProviderSelectionError::DisabledProvider { .. }
            | ProviderSelectionError::NoEnabledProviders
            | ProviderSelectionError::UnknownProvider { .. } => None,
        }
    }
}

fn initial_selected_index(providers: &[ProviderOption]) -> usize {
    providers
        .iter()
        .position(ProviderOption::is_active)
        .or_else(|| providers.iter().position(ProviderOption::is_enabled))
        .unwrap_or(0)
}

fn provider_list_items(providers: &[ProviderOption]) -> Vec<ListItem<'_>> {
    providers
        .iter()
        .map(|provider| ListItem::new(provider_line(provider)))
        .collect()
}

fn provider_line(provider: &ProviderOption) -> Line<'_> {
    let marker = if provider.active { "[x]" } else { "[ ]" };

    if !provider.enabled {
        return Line::from(vec![
            Span::raw(format!("{marker} {:<18}", provider.display_name)),
            Span::styled("Disabled", Style::default().add_modifier(Modifier::DIM)),
        ]);
    }

    let key_indicator = provider
        .masked_api_key
        .as_ref()
        .map(|key| format!("(key: {key})"))
        .unwrap_or_default();

    Line::from(format!(
        "{marker} {:<18}{key_indicator}",
        provider.display_name
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    #[test]
    fn masks_api_keys_without_exposing_full_key() {
        let key = "sk-or-v1-1de08abcdefghijklmnopqrstuvwxyz";
        let masked = mask_api_key(key).unwrap();

        assert!(masked.starts_with("sk-or-v1-1de"));
        assert!(masked.ends_with("**************************"));
        assert!(!masked.contains("abcdefghijklmnopqrstuvwxyz"));
    }

    #[test]
    fn blank_api_keys_do_not_render_indicators() {
        assert_eq!(mask_api_key("   "), None);
    }

    #[test]
    fn short_api_keys_are_not_displayed_in_full() {
        let key = "short";
        let masked = mask_api_key(key).unwrap();

        assert_ne!(masked, key);
        assert!(!masked.contains(key));
    }

    #[test]
    fn enabled_provider_can_be_selected() {
        let providers = provider_options();

        let selected = select_provider_id(&providers, "openrouter").unwrap();

        assert_eq!(selected, "openrouter");
    }

    #[test]
    fn disabled_provider_returns_clear_error() {
        let providers = provider_options();

        let error = select_provider_id(&providers, "openai").unwrap_err();

        assert!(matches!(
            error,
            ProviderSelectionError::DisabledProvider { ref provider_name } if provider_name == "OpenAI"
        ));
        assert!(error.to_string().contains("disabled"));
    }

    #[test]
    fn provider_screen_renders_title_and_rows() {
        let backend = TestBackend::new(80, 16);
        let mut terminal = Terminal::new(backend).unwrap();
        let providers = provider_options();

        terminal
            .draw(|frame| render_provider_selection(frame, &providers, 0, None))
            .unwrap();

        let rendered = rendered_text(&terminal);

        assert!(rendered.contains("Spectacular Providers"));
        assert!(rendered.contains("OpenRouter"));
        assert!(rendered.contains("OpenAI"));
        assert!(rendered.contains("Disabled"));
        assert_has_no_section_chrome(&rendered);
    }

    fn provider_options() -> Vec<ProviderOption> {
        vec![
            ProviderOption::new(
                "openrouter",
                "OpenRouter",
                true,
                Some("sk-or-v1-1de08abcdefghijklmnopqrstuvwxyz"),
                false,
            ),
            ProviderOption::new("openai", "OpenAI", false, None, false),
        ]
    }

    fn rendered_text(terminal: &Terminal<TestBackend>) -> String {
        terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .fold(String::new(), |mut output, cell| {
                output.push_str(cell.symbol());
                output
            })
    }

    fn assert_has_no_section_chrome(rendered: &str) {
        for border in ["â”Œ", "â”", "â””", "â”˜", "â”€", "â”‚"] {
            assert!(!rendered.contains(border));
        }

        for title in ["Configuration", "Assignments", "API key"] {
            assert!(!rendered.contains(title));
        }
    }
}
