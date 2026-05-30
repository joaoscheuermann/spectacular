//! Worker-local registration for shared built-in tools.

use agent::{ToolRegistrationError, ToolStorage};

use crate::repo::WorkerLayout;

/// Builds the shared built-in tool storage for a prepared worker layout.
pub fn worker_tool_storage(layout: &WorkerLayout) -> Result<ToolStorage, ToolRegistrationError> {
    tools::built_in_tools_with_trace_dir(layout.repo(), layout.tool_output())
}
