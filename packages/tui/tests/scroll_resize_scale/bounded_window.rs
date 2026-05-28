use super::*;

#[test]
fn scroll_state_when_large_transcript_render_uses_bounded_visible_window() {
    let mut state = state();
    populate_large_transcript(&mut state);

    let (output, elapsed) = timed_render(&state);

    assert!(elapsed < RENDER_BUDGET, "large render took {elapsed:?}");
    assert!(output.contains(&format!(
        "large transcript item {}",
        LARGE_TRANSCRIPT_ITEMS - 1
    )));
    assert!(!output.contains("large transcript item 0"));
}

/// Verifies the runtime App path uses the same bounded transcript window as test rendering.
#[test]
fn scroll_state_when_runtime_app_render_uses_bounded_visible_window() {
    let mut state = state();
    populate_large_transcript(&mut state);

    let output = render_state_to_string(&state, Some(120));

    assert!(output.contains(&format!(
        "large transcript item {}",
        LARGE_TRANSCRIPT_ITEMS - 1
    )));
    assert!(!output.contains("large transcript item 0"));
}

/// Verifies rendered-selection projection materializes only the visible transcript window.
#[test]
fn scroll_state_when_large_transcript_selection_projection_stays_windowed() {
    let mut state = state();
    populate_large_transcript(&mut state);

    let projection = SelectableProjection::for_state(&state);
    let transcript_rows = projection
        .rows()
        .iter()
        .filter(|row| row.surface == SelectableSurface::Transcript)
        .collect::<Vec<_>>();

    assert!(transcript_rows.len() <= usize::from(VISIBLE_TRANSCRIPT_ROWS));
    assert!(transcript_rows.iter().any(|row| row.text.contains(&format!(
        "large transcript item {}",
        LARGE_TRANSCRIPT_ITEMS - 1
    ))));
    assert!(!transcript_rows
        .iter()
        .any(|row| row.text.contains("large transcript item 0")));

    let row = u16::try_from(
        transcript_rows
            .iter()
            .find(|row| row.text.starts_with("large transcript item"))
            .unwrap()
            .screen_row,
    )
    .unwrap();
    state.app_selection = tui::selection::start_selection_at(&state, 0, row).unwrap();
    state.app_selection = tui::selection::drag_selection_to(&state, 4, row).unwrap();

    let selected = tui::selected_text(&state).unwrap();
    assert_eq!(selected, "larg");
}
