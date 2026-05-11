use spectacular_agent::{ToolRegistrationError, ToolStorage};
use std::path::PathBuf;

mod diff_preview;
mod display;
pub mod edit;
pub mod find;
mod fs_helpers;
pub mod grep;
mod output_preview;
pub mod path;
pub mod terminal;
#[cfg(test)]
#[path = "../tests/support/mod.rs"]
mod test_support;
pub mod tree;
pub mod web;
pub mod write;

pub use edit::{EditTool, EDIT_TOOL_NAME};
pub use find::{FindTool, FIND_TOOL_NAME};
pub use grep::{GrepTool, GREP_TOOL_NAME};
pub use terminal::{TerminalTool, TERMINAL_TOOL_NAME};
pub use tree::{TreeTool, TREE_TOOL_NAME};
pub use web::{WebSearchTool, WEB_SEARCH_TOOL_NAME};
pub use write::{WriteTool, WRITE_TOOL_NAME};

/// Registers all built-in tools against a shared workspace root and returns tool storage.
pub fn built_in_tools(
    workspace_root: impl Into<PathBuf>,
) -> Result<ToolStorage, ToolRegistrationError> {
    let workspace_root = workspace_root.into();
    let mut storage = ToolStorage::default();
    storage.register(EditTool::new(workspace_root.clone()))?;
    storage.register(FindTool::new(workspace_root.clone()))?;
    storage.register(GrepTool::new(workspace_root.clone()))?;
    storage.register(TerminalTool::new(workspace_root.clone()))?;
    storage.register(TreeTool::new(workspace_root.clone()))?;
    storage.register(WebSearchTool)?;
    storage.register(WriteTool::new(workspace_root))?;
    Ok(storage)
}

#[cfg(test)]
mod tests {
    include!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/unit/lib.rs"));
}
