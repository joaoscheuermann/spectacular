use crate::chat::ChatError;
use crate::chat::commands::{
    CompletionCommandSpec, CompletionEnvironment, CompletionFieldSpec, CompletionSubcommandSpec,
};
use crate::chat::model::{ChatModel, ChatPromptFooterModel};
use crate::chat::paste_burst::{CharDecision, FlushResult, PasteBurst};
use crate::chat::renderer::{Renderer, dim_style, paint, selection_style, title_style, user_style};
use anstyle::RgbColor;
use crossterm::cursor::{Hide, MoveDown, MoveToColumn, MoveUp, Show};
use crossterm::event::{
    self, DisableBracketedPaste, EnableBracketedPaste, Event, KeyCode, KeyEvent, KeyEventKind,
    KeyModifiers,
};
use crossterm::queue;
use crossterm::terminal::{self, Clear, ClearType, disable_raw_mode, enable_raw_mode};
use spectacular_commands::{CommandRegistry, ParseOutcome, fuzzy_filter, fuzzy_rank, parse_line};
use std::io::{self, Write};
use std::ops::Range;
use std::sync::Arc;
use std::time::Instant;
use unicode_width::UnicodeWidthChar;

const DEFAULT_TERMINAL_WIDTH: u16 = 80;
const MAX_SUGGESTIONS: usize = 8;
const PROMPT_WIDTH: u16 = 2;
const PROMPT_FOOTER_RENDERED_LINES: usize = 2;
const MISSING_ORANGE: RgbColor = RgbColor(251, 191, 36);

include!("prompt/editor_types.rs");
include!("prompt/editor.rs");
include!("prompt/terminal.rs");
include!("prompt/state.rs");
include!("prompt/layout.rs");
include!("prompt/completion.rs");
include!("prompt/completion_fields.rs");
include!("prompt/navigation.rs");
include!("prompt/selection.rs");

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chat::commands::{ChatCompletionContext, CompletionValueValidation};
    use spectacular_commands::{Command, CommandControl, CommandFuture};

    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/unit/chat/prompt_support.rs"
    ));
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/unit/chat/prompt_completion.rs"
    ));
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/unit/chat/prompt_editing.rs"
    ));
}
