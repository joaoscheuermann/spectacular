use crate::chat::commands::{
    ChatCommand, ChatCommandContext, ChatCommandFuture, ChatCommandResult,
};
use crate::chat::session::HistoryQuery;
use spectacular_commands::CommandError;

pub fn command() -> ChatCommand {
    ChatCommand {
        name: "history",
        usage: "/history [page|start-end]",
        summary: "List saved sessions",
        completion: &[],
        execute,
    }
}

fn execute<'a>(context: ChatCommandContext<'a>, args: Vec<String>) -> ChatCommandFuture<'a> {
    Box::pin(async move {
        let query = match parse_query(&args) {
            Ok(query) => query,
            Err(error) => return ChatCommandResult::error(error.to_string()),
        };
        let table = match context.model.history(query) {
            Ok(table) => table,
            Err(error) => return ChatCommandResult::error(error.to_string()),
        };

        context.render_history(&table);

        ChatCommandResult::success()
    })
}

fn parse_query(args: &[String]) -> Result<HistoryQuery, CommandError> {
    match args {
        [] => Ok(HistoryQuery::FirstPage),
        [value] if value.contains('-') => {
            let Some((start, end)) = value.split_once('-') else {
                return Err(CommandError::usage("/history [page|start-end]"));
            };
            Ok(HistoryQuery::Range(parse_usize(start)?, parse_usize(end)?))
        }
        [value] => Ok(HistoryQuery::Page(parse_usize(value)?)),
        _ => Err(CommandError::usage("/history [page|start-end]")),
    }
}

fn parse_usize(value: &str) -> Result<usize, CommandError> {
    value
        .parse::<usize>()
        .map_err(|_| CommandError::usage("/history [page|start-end]"))
}

#[cfg(test)]
mod tests {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/unit/chat/commands/session/history.rs"
    ));
}
