use super::*;

#[test]
fn normal_text_is_not_command() {
    assert_eq!(parse_line("hello").unwrap(), ParseOutcome::NotCommand);
}

#[test]
fn empty_slash_is_rejected() {
    assert!(matches!(parse_line("/"), Err(CommandError::EmptyCommand)));
    assert!(matches!(
        parse_line("/   "),
        Err(CommandError::EmptyCommand)
    ));
}

#[test]
fn command_with_quoted_args_parses() {
    let parsed = parse_line(r#"/model add id:"openai/gpt 4" reasoning:medium"#).unwrap();

    assert_eq!(
        parsed,
        ParseOutcome::Command(CommandInvocation {
            name: "model".to_owned(),
            args: vec![
                "add".to_owned(),
                "id:openai/gpt 4".to_owned(),
                "reasoning:medium".to_owned()
            ]
        })
    );
}

#[test]
fn escaped_quotes_parse_inside_quotes() {
    let parsed = parse_line(r#"/echo "say \"hello\"""#).unwrap();

    assert_eq!(
        parsed,
        ParseOutcome::Command(CommandInvocation {
            name: "echo".to_owned(),
            args: vec![r#"say "hello""#.to_owned()]
        })
    );
}

#[test]
fn unterminated_quote_is_rejected() {
    assert!(matches!(
        parse_line(r#"/model "openrouter/model"#),
        Err(CommandError::UnterminatedQuote)
    ));
}

#[test]
fn uppercase_command_is_unknown() {
    assert!(matches!(
        parse_line("/History"),
        Err(CommandError::UnknownCommand { name }) if name == "History"
    ));
}

#[test]
fn duplicate_registration_fails() {
    #[derive(Default)]
    struct Context;

    fn execute<'a>(_context: &'a mut Context, _args: Vec<String>) -> CommandFuture<'a> {
        Box::pin(async { Ok(CommandControl::Continue) })
    }

    let command = Command {
        name: "test",
        usage: "/test",
        summary: "test",
        execute,
    };
    let mut registry = CommandRegistry::<Context>::new();
    registry.register(command).unwrap();

    assert!(matches!(
        registry.register(command),
        Err(CommandError::DuplicateCommand { name }) if name == "test"
    ));
}

#[test]
fn search_empty_query_returns_all_commands() {
    let registry = test_registry();
    let matches = registry.search("", 8);

    assert_eq!(
        matches
            .iter()
            .map(|entry| entry.metadata.name)
            .collect::<Vec<_>>(),
        vec!["clear", "history", "model", "resume"]
    );
}

#[test]
fn search_ranks_prefix_before_levenshtein() {
    let registry = registry_with([("haiku", "Write haiku"), ("history", "List sessions")]);
    let matches = registry.search("hi", 8);

    assert_eq!(matches[0].metadata.name, "history");
    assert!(!matches.iter().any(|entry| entry.metadata.name == "haiku"));
}

#[test]
fn search_matches_close_levenshtein_typos() {
    let registry = test_registry();
    let matches = registry.search("histroy", 8);

    assert_eq!(
        matches
            .iter()
            .map(|entry| entry.metadata.name)
            .collect::<Vec<_>>(),
        vec!["history"]
    );
}

#[test]
fn search_is_case_sensitive() {
    let registry = test_registry();

    assert!(registry.search("History", 8).is_empty());
}

#[test]
fn named_args_parse_colon_fields_and_quoted_values() {
    let parsed = parse_line(r#"/provider add name:"work key" type:openrouter"#).unwrap();
    let ParseOutcome::Command(invocation) = parsed else {
        panic!("expected command");
    };

    let named = NamedArgs::parse(&invocation.args[1..]).unwrap();

    assert_eq!(named.require("name").unwrap(), "work key");
    assert_eq!(named.require("type").unwrap(), "openrouter");
}

#[test]
fn named_args_reject_duplicates() {
    let args = vec!["name:one".to_owned(), "name:two".to_owned()];

    assert!(matches!(
        NamedArgs::parse(&args),
        Err(CommandError::DuplicateNamedArgument { name }) if name == "name"
    ));
}

#[test]
fn fuzzy_filter_uses_stable_tie_breaking() {
    let matches = fuzzy_filter(["gpt-5", "gpt-4", "gemini"].iter().copied(), "gpt", 8);

    assert_eq!(matches, vec!["gpt-4", "gpt-5"]);
}

fn test_registry() -> CommandRegistry<()> {
    registry_with([
        ("clear", "Clear terminal"),
        ("history", "List sessions"),
        ("model", "Set model"),
        ("resume", "Resume session"),
    ])
}

fn registry_with<const N: usize>(
    entries: [(&'static str, &'static str); N],
) -> CommandRegistry<()> {
    fn execute<'a>(_context: &'a mut (), _args: Vec<String>) -> CommandFuture<'a> {
        Box::pin(async { Ok(CommandControl::Continue) })
    }

    let mut registry = CommandRegistry::new();
    for (name, summary) in entries {
        registry
            .register(Command {
                name,
                usage: name,
                summary,
                execute,
            })
            .unwrap();
    }
    registry
}
