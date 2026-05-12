use crate::chat::renderer::{has_visible_assistant_text, has_visible_reasoning_text};

/// Tracks whether streamed assistant text is visible or still pending whitespace.
#[derive(Default)]
pub(super) struct AssistantResponseRenderState {
    pending: String,
    visible: bool,
}

/// Renderable assistant delta plus whether it starts a visible response.
pub(super) struct AssistantDeltaRender {
    pub(super) content: String,
    pub(super) started: bool,
}

impl AssistantResponseRenderState {
    /// Returns newly visible assistant text once accumulated deltas contain nonblank content.
    pub(super) fn delta(&mut self, content: &str) -> Option<AssistantDeltaRender> {
        if self.visible {
            return Some(AssistantDeltaRender {
                content: content.to_owned(),
                started: false,
            });
        }

        self.pending.push_str(content);
        if !has_visible_assistant_text(&self.pending) {
            return None;
        }

        self.visible = true;
        Some(AssistantDeltaRender {
            content: std::mem::take(&mut self.pending),
            started: true,
        })
    }

    /// Closes any visible response and reports whether a spacer line should be emitted.
    pub(super) fn close_visible_response(&mut self) -> bool {
        self.pending.clear();
        if !self.visible {
            return false;
        }

        self.visible = false;
        true
    }
}

/// Tracks whether streamed reasoning text is visible or still pending whitespace.
#[derive(Default)]
pub(super) struct ReasoningResponseRenderState {
    pending: String,
    visible: bool,
}

/// Renderable reasoning delta plus whether it starts a visible block.
pub(super) struct ReasoningDeltaRender {
    pub(super) content: String,
    pub(super) started: bool,
}

impl ReasoningResponseRenderState {
    /// Returns newly visible reasoning text once accumulated deltas contain nonblank content.
    pub(super) fn delta(&mut self, content: &str) -> Option<ReasoningDeltaRender> {
        if self.visible {
            return Some(ReasoningDeltaRender {
                content: content.to_owned(),
                started: false,
            });
        }

        self.pending.push_str(content);
        if !has_visible_reasoning_text(&self.pending) {
            return None;
        }

        self.visible = true;
        Some(ReasoningDeltaRender {
            content: std::mem::take(&mut self.pending),
            started: true,
        })
    }

    /// Closes any visible reasoning block and reports whether a spacer line should be emitted.
    pub(super) fn close_visible_response(&mut self) -> bool {
        self.pending.clear();
        if !self.visible {
            return false;
        }

        self.visible = false;
        true
    }
}
