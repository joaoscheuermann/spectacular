use crate::keys::is_cancel_key;
use crate::navigation::{next_index, previous_index};
use crate::terminal::TerminalSession;
use crate::widgets::input::Input;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::Line,
    widgets::{
        Block, Borders, List, ListItem, ListState, Paragraph, Scrollbar, ScrollbarOrientation,
        ScrollbarState, Wrap,
    },
    Frame,
};
use std::error::Error;
use std::fmt::{self, Display};
use std::io;

pub const TASK_MODEL_SLOT_COUNT: usize = 3;
const TASK_MODEL_SLOT_NAMES: [&str; TASK_MODEL_SLOT_COUNT] = ["planning", "labeling", "coding"];

/// Provider model row used by the model assignment screen.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModelOption {
    id: String,
    display_name: String,
}

impl ModelOption {
    pub fn new(id: impl Into<String>, display_name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            display_name: display_name.into(),
        }
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn display_name(&self) -> &str {
        &self.display_name
    }
}

/// Model IDs assigned to all required Spectacular task slots.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TaskModelSelection {
    planning: String,
    labeling: String,
    coding: String,
}

impl TaskModelSelection {
    pub fn new(
        planning: impl Into<String>,
        labeling: impl Into<String>,
        coding: impl Into<String>,
    ) -> Self {
        Self {
            planning: planning.into(),
            labeling: labeling.into(),
            coding: coding.into(),
        }
    }

    pub fn planning(&self) -> &str {
        &self.planning
    }

    pub fn labeling(&self) -> &str {
        &self.labeling
    }

    pub fn coding(&self) -> &str {
        &self.coding
    }
}

/// Runs the interactive Ratatui model assignment screen for all task slots.
pub fn run_model_assignment_screen(
    provider_name: &str,
    models: &[ModelOption],
    current_selection: Option<TaskModelSelection>,
) -> Result<TaskModelSelection, ModelAssignmentError> {
    if models.is_empty() {
        return Err(ModelAssignmentError::NoModelsAvailable {
            provider_name: provider_name.to_owned(),
        });
    }

    let mut session = TerminalSession::start().map_err(ModelAssignmentError::TerminalIo)?;
    let mut selected_slot = 0;
    let mut model_indices = initial_model_assignment_indices(models, current_selection.as_ref());
    let mut search_query = String::new();
    let mut status_message: Option<String> = None;

    loop {
        session.terminal.draw(|frame| {
            render_model_assignment_screen(
                frame,
                provider_name,
                models,
                model_indices,
                selected_slot,
                &search_query,
                status_message.as_deref(),
            );
        })?;

        let Event::Key(key) = event::read()? else {
            continue;
        };

        if key.kind != KeyEventKind::Press {
            continue;
        }

        if is_cancel_key(&key.code) {
            return Err(ModelAssignmentError::Cancelled);
        }

        match key.code {
            KeyCode::Up => {
                status_message = move_selected_model(
                    models,
                    &search_query,
                    &mut model_indices,
                    selected_slot,
                    previous_index,
                );
            }
            KeyCode::Down => {
                status_message = move_selected_model(
                    models,
                    &search_query,
                    &mut model_indices,
                    selected_slot,
                    next_index,
                );
            }
            KeyCode::Left => {
                selected_slot = previous_index(selected_slot, TASK_MODEL_SLOT_COUNT);
                status_message = None;
            }
            KeyCode::Right | KeyCode::Tab => {
                selected_slot = next_index(selected_slot, TASK_MODEL_SLOT_COUNT);
                status_message = None;
            }
            KeyCode::Enter => {
                return Ok(selection_from_indices(models, model_indices));
            }
            KeyCode::Char(character) => {
                search_query.push(character);
                status_message = apply_first_filtered_model(
                    models,
                    &search_query,
                    &mut model_indices,
                    selected_slot,
                );
            }
            KeyCode::Backspace => {
                search_query.pop();
                status_message = apply_first_filtered_model(
                    models,
                    &search_query,
                    &mut model_indices,
                    selected_slot,
                );
            }
            _ => {}
        }
    }
}

/// Renders all task model assignments into one Ratatui screen.
pub fn render_model_assignment_screen(
    frame: &mut Frame<'_>,
    provider_name: &str,
    models: &[ModelOption],
    model_indices: [usize; TASK_MODEL_SLOT_COUNT],
    selected_slot: usize,
    search_query: &str,
    status_message: Option<&str>,
) {
    let selected_slot = selected_slot.min(TASK_MODEL_SLOT_COUNT - 1);
    let areas = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(2),
            Constraint::Length(4),
            Constraint::Length(3),
            Constraint::Min(5),
            Constraint::Length(1),
            Constraint::Length(2),
        ])
        .split(frame.area());

    frame.render_widget(
        Paragraph::new(format!("Spectacular {provider_name} Models"))
            .style(Style::default().fg(Color::Blue)),
        areas[0],
    );

    frame.render_widget(
        Paragraph::new("Assign models for planning, labeling, and coding tasks.")
            .wrap(Wrap { trim: true }),
        areas[1],
    );

    let assignment_lines = TASK_MODEL_SLOT_NAMES
        .iter()
        .enumerate()
        .map(|(slot_index, slot_name)| {
            assignment_line(
                slot_name,
                models,
                model_indices[slot_index],
                slot_index == selected_slot,
            )
        })
        .collect::<Vec<_>>();
    frame.render_widget(Paragraph::new(assignment_lines), areas[2]);

    let search = Input::new(search_query)
        .focused(true)
        .block(Block::default().borders(Borders::ALL).title(" Search "));
    frame.render_widget(search, areas[3]);

    let filtered_model_indices = filtered_model_indices(models, search_query);
    let selected_model_index = model_indices[selected_slot].min(models.len().saturating_sub(1));
    let selected_filtered_index = filtered_model_indices
        .iter()
        .position(|model_index| *model_index == selected_model_index)
        .unwrap_or(0);
    let items = filtered_model_indices
        .iter()
        .map(|model_index| {
            let model = &models[*model_index];
            ListItem::new(Line::from(format!(
                "{} ({})",
                model.display_name(),
                model.id()
            )))
        })
        .collect::<Vec<_>>();
    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(" Models "))
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED));
    let mut state = ListState::default();
    state.select((!filtered_model_indices.is_empty()).then_some(selected_filtered_index));
    frame.render_stateful_widget(list, areas[4], &mut state);

    let mut scrollbar_state =
        ScrollbarState::new(filtered_model_indices.len()).position(state.offset());
    frame.render_stateful_widget(
        Scrollbar::new(ScrollbarOrientation::VerticalRight),
        areas[4],
        &mut scrollbar_state,
    );

    let status = status_message.unwrap_or(
        "Type to search, Up/Down to choose, Tab to switch tasks, Enter to save, Esc to cancel.",
    );
    frame.render_widget(Paragraph::new(status).wrap(Wrap { trim: true }), areas[6]);
}

/// Errors returned by the model assignment UI.
#[derive(Debug)]
pub enum ModelAssignmentError {
    Cancelled,
    NoModelsAvailable { provider_name: String },
    TerminalIo(io::Error),
}

impl From<io::Error> for ModelAssignmentError {
    fn from(source: io::Error) -> Self {
        Self::TerminalIo(source)
    }
}

impl Display for ModelAssignmentError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ModelAssignmentError::Cancelled => {
                formatter.write_str("Model assignment was cancelled.")
            }
            ModelAssignmentError::NoModelsAvailable { provider_name } => {
                write!(formatter, "{provider_name} returned no models to assign.")
            }
            ModelAssignmentError::TerminalIo(source) => {
                write!(formatter, "Model assignment failed: {source}")
            }
        }
    }
}

impl Error for ModelAssignmentError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            ModelAssignmentError::TerminalIo(source) => Some(source),
            ModelAssignmentError::Cancelled | ModelAssignmentError::NoModelsAvailable { .. } => {
                None
            }
        }
    }
}

fn initial_model_assignment_indices(
    models: &[ModelOption],
    current_selection: Option<&TaskModelSelection>,
) -> [usize; TASK_MODEL_SLOT_COUNT] {
    let Some(current_selection) = current_selection else {
        return [0; TASK_MODEL_SLOT_COUNT];
    };

    [
        model_index(models, current_selection.planning()),
        model_index(models, current_selection.labeling()),
        model_index(models, current_selection.coding()),
    ]
}

fn model_index(models: &[ModelOption], model_id: &str) -> usize {
    models
        .iter()
        .position(|model| model.id() == model_id)
        .unwrap_or(0)
}

fn selection_from_indices(
    models: &[ModelOption],
    model_indices: [usize; TASK_MODEL_SLOT_COUNT],
) -> TaskModelSelection {
    TaskModelSelection::new(
        models[model_indices[0]].id(),
        models[model_indices[1]].id(),
        models[model_indices[2]].id(),
    )
}

fn move_selected_model(
    models: &[ModelOption],
    search_query: &str,
    model_indices: &mut [usize; TASK_MODEL_SLOT_COUNT],
    selected_slot: usize,
    next_position: fn(usize, usize) -> usize,
) -> Option<String> {
    let filtered_indices = filtered_model_indices(models, search_query);

    if filtered_indices.is_empty() {
        return Some("No models match the current search.".to_owned());
    }

    let current_model_index = model_indices[selected_slot];
    let current_filtered_index = filtered_indices
        .iter()
        .position(|model_index| *model_index == current_model_index)
        .unwrap_or(0);
    let next_filtered_index = next_position(current_filtered_index, filtered_indices.len());
    model_indices[selected_slot] = filtered_indices[next_filtered_index];

    None
}

fn apply_first_filtered_model(
    models: &[ModelOption],
    search_query: &str,
    model_indices: &mut [usize; TASK_MODEL_SLOT_COUNT],
    selected_slot: usize,
) -> Option<String> {
    let Some(model_index) = filtered_model_indices(models, search_query)
        .first()
        .copied()
    else {
        return Some("No models match the current search.".to_owned());
    };

    model_indices[selected_slot] = model_index;
    None
}

fn filtered_model_indices(models: &[ModelOption], search_query: &str) -> Vec<usize> {
    let query = search_query.trim();

    if query.is_empty() {
        return (0..models.len()).collect();
    }

    let mut scored_indices = models
        .iter()
        .enumerate()
        .filter_map(|(index, model)| {
            let search_text = format!("{} {}", model.display_name(), model.id());
            fuzzy_score(&search_text, query).map(|score| (index, score))
        })
        .collect::<Vec<_>>();
    scored_indices.sort_by_key(|(index, score)| (*score, *index));

    scored_indices.into_iter().map(|(index, _)| index).collect()
}

fn fuzzy_score(candidate: &str, query: &str) -> Option<usize> {
    let mut score = 0;
    let mut candidate_chars = candidate.char_indices();
    let mut last_match_index = None;

    for query_char in query.chars().flat_map(char::to_lowercase) {
        let Some((match_index, _)) = candidate_chars.find(|(_, candidate_char)| {
            candidate_char
                .to_lowercase()
                .any(|candidate_lower| candidate_lower == query_char)
        }) else {
            return None;
        };

        score += match last_match_index {
            Some(last_index) => match_index.saturating_sub(last_index + 1),
            None => match_index,
        };
        last_match_index = Some(match_index);
    }

    Some(score)
}

fn assignment_line(
    slot_name: &str,
    models: &[ModelOption],
    model_index: usize,
    selected: bool,
) -> Line<'static> {
    let marker = if selected { ">" } else { " " };
    let model = &models[model_index.min(models.len().saturating_sub(1))];

    Line::from(format!(
        "{marker} {:<9} {} ({})",
        slot_name,
        model.display_name(),
        model.id()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    #[test]
    fn model_assignment_screen_renders_all_task_slots_and_models() {
        let backend = TestBackend::new(100, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let models = model_options();

        terminal
            .draw(|frame| {
                render_model_assignment_screen(
                    frame,
                    "OpenRouter",
                    &models,
                    [0, 1, 0],
                    1,
                    "",
                    None,
                );
            })
            .unwrap();

        let rendered = rendered_text(&terminal);

        assert!(rendered.contains("Spectacular OpenRouter Models"));
        assert!(rendered.contains("Assign models for planning, labeling, and coding tasks."));
        assert!(rendered.contains("Search"));
        assert!(rendered.contains("Models"));
        assert!(rendered.contains("planning"));
        assert!(rendered.contains("labeling"));
        assert!(rendered.contains("coding"));
        assert!(rendered.contains("GPT-4o"));
        assert!(rendered.contains("Claude Sonnet"));
        assert_has_no_unrelated_section_titles(&rendered);
    }

    #[test]
    fn model_assignment_screen_filters_models_by_search_query() {
        let backend = TestBackend::new(100, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let models = model_options();

        terminal
            .draw(|frame| {
                render_model_assignment_screen(
                    frame,
                    "OpenRouter",
                    &models,
                    [1, 1, 1],
                    1,
                    "claude",
                    None,
                );
            })
            .unwrap();

        let rendered = rendered_text(&terminal);

        assert!(rendered.contains("Claude Sonnet"));
        assert!(!rendered.contains("GPT-4o"));
    }

    #[test]
    fn model_assignment_initializes_from_current_selection() {
        let models = model_options();
        let current = TaskModelSelection::new(
            "anthropic/claude-sonnet-4.5",
            "openai/gpt-4o",
            "missing/model",
        );

        let indices = initial_model_assignment_indices(&models, Some(&current));

        assert_eq!(indices, [1, 0, 0]);
    }

    #[test]
    fn model_assignment_selection_uses_all_three_slots() {
        let models = model_options();
        let selection = selection_from_indices(&models, [1, 0, 1]);

        assert_eq!(selection.planning(), "anthropic/claude-sonnet-4.5");
        assert_eq!(selection.labeling(), "openai/gpt-4o");
        assert_eq!(selection.coding(), "anthropic/claude-sonnet-4.5");
    }

    #[test]
    fn model_assignment_requires_models() {
        let error = run_model_assignment_screen("OpenRouter", &[], None).unwrap_err();

        assert!(matches!(
            error,
            ModelAssignmentError::NoModelsAvailable { ref provider_name }
                if provider_name == "OpenRouter"
        ));
    }

    #[test]
    fn fuzzy_search_matches_model_name_or_id() {
        let models = model_options();

        assert_eq!(filtered_model_indices(&models, "sonnet"), vec![1]);
        assert_eq!(filtered_model_indices(&models, "gpt4"), vec![0]);
    }

    #[test]
    fn search_updates_current_slot_to_first_match() {
        let models = model_options();
        let mut indices = [0, 0, 0];

        let status = apply_first_filtered_model(&models, "claude", &mut indices, 2);

        assert_eq!(status, None);
        assert_eq!(indices, [0, 0, 1]);
    }

    #[test]
    fn search_reports_no_matching_models() {
        let models = model_options();
        let mut indices = [0, 0, 0];

        let status = apply_first_filtered_model(&models, "missing", &mut indices, 2);

        assert_eq!(
            status,
            Some("No models match the current search.".to_owned())
        );
        assert_eq!(indices, [0, 0, 0]);
    }

    fn model_options() -> Vec<ModelOption> {
        vec![
            ModelOption::new("openai/gpt-4o", "GPT-4o"),
            ModelOption::new("anthropic/claude-sonnet-4.5", "Claude Sonnet"),
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

    fn assert_has_no_unrelated_section_titles(rendered: &str) {
        for title in ["Configuration", "Assignments", "API key"] {
            assert!(!rendered.contains(title));
        }
    }

    #[allow(dead_code)]
    fn assert_has_no_section_chrome(rendered: &str) {
        for border in ["â”Œ", "â”", "â””", "â”˜", "â”€", "â”‚"] {
            assert!(!rendered.contains(border));
        }

        for title in ["Configuration", "Assignments", "API key"] {
            assert!(!rendered.contains(title));
        }
    }
}
