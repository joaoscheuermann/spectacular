use crate::chat::renderer::{dim_style, paint};
use crate::chat::session::{format_local_time, HistoryQuery};
use crate::chat::ChatContext;
use spectacular_commands::{Command, CommandControl, CommandError, CommandFuture};

pub fn command() -> Command<ChatContext> {
    Command {
        name: "history",
        usage: "/history [page|start-end]",
        summary: "List saved sessions",
        execute,
    }
}

fn execute<'a>(context: &'a mut ChatContext, args: Vec<String>) -> CommandFuture<'a> {
    Box::pin(async move {
        let query = parse_query(&args)?;
        let page = context
            .session
            .history(query)
            .map_err(|error| CommandError::message(error.to_string()))?;
        println!("sessions");
        println!("hash      updated           title                  messages");
        for session in page.sessions {
            let marker = if session.corrupt { "*" } else { " " };
            println!(
                "{:<8}  {:<16}  {:<22}  {}{}",
                session.id,
                format_local_time(session.updated),
                truncate(&session.title, 22),
                session.messages,
                marker
            );
        }
        if page.remaining > 0 {
            println!();
            println!(
                "{}",
                paint(dim_style(), format!("{} more sessions", page.remaining))
            );
        }
        Ok(CommandControl::Continue)
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

fn truncate(value: &str, limit: usize) -> String {
    if value.chars().count() <= limit {
        return value.to_owned();
    }

    value
        .chars()
        .take(limit.saturating_sub(3))
        .collect::<String>()
        + "..."
}
