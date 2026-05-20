# IOCraft Guide

`iocraft` is a Rust crate for declarative terminal UI, CLI output, fullscreen TUIs, and text-based interaction. It is conceptually similar to React or Ink, but designed for Rust and terminal rendering.

Use IOCraft when a feature needs structured terminal output, styled logs, interactive prompts, progress displays, forms, tables, or fullscreen terminal applications.

## Core model

IOCraft UIs are trees of elements. Elements describe components and their props before rendering. Components can be built-in or user-defined.

The usual stack is:

1. Declare UI with `element!`.
2. Use built-in components such as `View` and `Text` for layout and display.
3. Extract custom components with `#[component]` when UI has a meaningful boundary.
4. Add interactivity with hooks.
5. Render statically with `print`/`write` or dynamically with `render_loop`/`fullscreen`.

Most files using IOCraft should import:

```rust
use iocraft::prelude::*;
```

## The `element!` macro

`element!` is the primary API for declaring UI.

```rust
let mut ui = element! {
    View(
        border_style: BorderStyle::Round,
        border_color: Color::Blue,
        padding: 1,
    ) {
        Text(content: "Hello, world!")
    }
};

ui.print();
```

Supported patterns:

```rust
// Component with no props.
element!(View)

// Component with props.
element! {
    Text(content: "Status", color: Color::Green, weight: Weight::Bold)
}

// Parent with children.
element! {
    View {
        Text(content: "Line 1")
        Text(content: "Line 2")
    }
}

// Conditional child.
element! {
    View {
        #(if show_warning {
            Some(element! { Text(content: "Warning", color: Color::Yellow) })
        } else {
            None
        })
    }
}

// Iterator children.
element! {
    View(flex_direction: FlexDirection::Column) {
        #(items.iter().map(|item| element! {
            Text(key: item.id, content: item.label.clone())
        }))
    }
}
```

### Dynamic children and `key`

Use stable `key` values for repeated dynamic elements when the list can change length, order, or contents. Keys preserve component identity and internal state across renders.

Good:

```rust
element! {
    View {
        #(users.iter().map(|user| element! {
            UserRow(key: user.id, user)
        }))
    }
}
```

Acceptable without keys only when the list is static and child components hold no state.

## Built-in components

Prefer built-ins before custom low-level drawing.

### `View`

`View` is the main layout and styling primitive. It uses flexbox-style layout via `taffy`.

Common props:

- Layout: `width`, `height`, `min_width`, `max_width`, `flex_direction`, `flex_wrap`, `flex_grow`, `flex_shrink`, `align_items`, `align_content`, `justify_content`, `gap`, `row_gap`, `column_gap`.
- Spacing: `padding`, `padding_top`, `padding_right`, `padding_bottom`, `padding_left`, `margin`, `margin_top`, `margin_right`, `margin_bottom`, `margin_left`.
- Styling: `border_style`, `border_color`, `border_edges`, `background_color`.
- Overflow/position: `overflow`, `overflow_x`, `overflow_y`, `position`, `top`, `right`, `bottom`, `left`, `inset`.

```rust
element! {
    View(
        flex_direction: FlexDirection::Column,
        align_items: AlignItems::Center,
        justify_content: JustifyContent::Center,
        padding: 2,
        border_style: BorderStyle::Round,
        border_color: Color::Cyan,
    ) {
        Text(content: "Centered panel")
    }
}
```

Use percentage sizes with the `pct` suffix or `Percent`:

```rust
element! {
    View(width: 100pct) {
        View(width: 30pct) { Text(content: "Left") }
        View(width: 70pct) { Text(content: "Right") }
    }
}
```

### `Text`

`Text` renders styled text.

Common props:

- `content`
- `color`
- `weight`
- `wrap`
- `align`
- `decoration`
- `italic`
- `invert`

```rust
element! {
    Text(
        content: "Complete",
        color: Color::Green,
        weight: Weight::Bold,
    )
}
```

### `MixedText`

Use `MixedText` when a single line or paragraph needs multiple styles. Prefer it over awkwardly splitting a sentence into many sibling `Text` nodes if the text should behave like one text block.

### `TextInput`

Use `TextInput` for editable input. It is usually controlled by state.

```rust
#[component]
fn NameField(mut hooks: Hooks) -> impl Into<AnyElement<'static>> {
    let mut name = hooks.use_state(|| String::new());

    element! {
        View(width: 30, background_color: Color::DarkGrey) {
            TextInput(
                value: name.to_string(),
                has_focus: true,
                on_change: move |new_value| name.set(new_value),
            )
        }
    }
}
```

Useful props:

- `value`
- `has_focus`
- `on_change`
- `multiline`
- `color`
- `cursor_color`
- `handle`

### `Button`

Use `Button` when a child should trigger a handler via mouse click or Enter/Space while focused.

```rust
element! {
    Button(has_focus: focused, handler: move |_| submitted.set(true)) {
        Text(content: "Submit", weight: Weight::Bold)
    }
}
```

### `ScrollView`

Use `ScrollView` for content that exceeds its visible region. Pair it with a constrained parent `View`.

```rust
element! {
    View(width: 80, height: 10, border_style: BorderStyle::Round) {
        ScrollView {
            Text(content: long_text)
        }
    }
}
```

### `Fragment`

Use `Fragment` to group children without affecting layout.

### `ContextProvider`

Use `ContextProvider` for data needed by many descendants without prop drilling.

## Custom components

Create custom components with `#[component]`. Components return something convertible into `AnyElement`.

```rust
#[component]
fn Header() -> impl Into<AnyElement<'static>> {
    element! {
        View(padding_bottom: 1) {
            Text(content: "My App", weight: Weight::Bold, color: Color::Cyan)
        }
    }
}
```

Components may take `hooks`, `props`, or both. The argument names matter.

```rust
#[derive(Default, Props)]
struct HeaderProps {
    title: String,
}

#[component]
fn Header(props: &HeaderProps) -> impl Into<AnyElement<'static>> {
    element! {
        Text(content: props.title.clone(), weight: Weight::Bold)
    }
}
```

### Borrowed props

IOCraft supports borrowed props well. Prefer borrowing domain data when it avoids unnecessary cloning.

```rust
#[derive(Default, Props)]
struct UsersTableProps<'a> {
    users: Option<&'a [User]>,
}

#[component]
fn UsersTable<'a>(props: &UsersTableProps<'a>) -> impl Into<AnyElement<'a>> {
    element! {
        View(flex_direction: FlexDirection::Column) {
            #(props.users.into_iter().flatten().map(|user| element! {
                Text(key: user.id, content: user.name.clone())
            }))
        }
    }
}
```

### Required props pattern

Because props often use `Option<T>` to distinguish omitted values, unwrap required props at the top of the component with a clear panic or early fallback.

```rust
#[derive(Default, Props)]
struct LabelProps<'a> {
    text: Option<&'a str>,
}

#[component]
fn Label<'a>(props: &LabelProps<'a>) -> impl Into<AnyElement<'a>> {
    let Some(text) = props.text else {
        panic!("text is required");
    };

    element! {
        Text(content: text)
    }
}
```

Use this sparingly. If a component can have a sensible default, prefer that instead.

### Component decomposition

Extract a component when it:

- Encapsulates repeated layout or styling.
- Owns state or terminal event handling.
- Represents a named UI concept, such as `Header`, `StatusBar`, `UserRow`, or `FormField`.
- Simplifies tests by isolating behavior.

Avoid splitting every `View` into a component; shallow components can make IOCraft UIs harder to read.

## Spectacular TUI transcript rendering

Repository invariant: Spectacular TUI rendering is component-first. When rendering visible transcript items, status rows, prompt rows, or other terminal lines in `packages/spectacular-tui`, prefer meaningful IOCraft components from `src/components/*` and compose them with `element!`.

Good component boundaries for this codebase include named transcript concepts such as user prompts, assistant messages, tool calls, command output, reasoning, notices, warnings, success, cancellation, worked summaries, the opening banner, prompt area, footer, and scrollable transcript content. Use the existing component for the concept when one exists. If the UI concept is missing, add or improve that component instead of adding another line-formatting function.

Avoid shallow rendering wrappers that only convert domain data into `Vec<RenderLine>`, `Vec<String>`, or equivalent terminal-flow rows. These flattening helpers hide UI structure, duplicate component behavior, make layout harder to evolve, and become bypasses around IOCraft. `packages/spectacular-tui/src/format.rs::transcript_item_render_lines` is the reference bad pattern: a broad match over transcript item variants that delegates to ad hoc per-kind line formatters and returns line vectors. Keep that shape as legacy compatibility only; do not copy it, expand it for new UI, or route new TUI rendering through it.

Use line-vector output only at narrow compatibility boundaries, snapshots that deliberately assert plain text, or adapters that are actively being retired. New rendering behavior should preserve semantic structure as IOCraft elements until the final render mode (`to_string`, `write`, `render_loop`, or `fullscreen`).

## Rendering modes

Choose the rendering mode based on the output lifecycle.

### Static output

Use for command output, status reports, logs, and generated text.

```rust
element!(Report).print();
element!(Report).eprint();
let rendered = element!(Report).to_string();
element!(Report).write(writer)?;
```

Static rendering is best when the UI does not need input or live updates.

### Inline dynamic rendering

Use `render_loop` for live progress, inline interactive prompts, counters, or non-fullscreen widgets.

```rust
smol::block_on(element!(ProgressBar).render_loop()).unwrap();
```

Use only when stdio is a TTY. If output is redirected to a file, prefer static output.

### Fullscreen rendering

Use `fullscreen` for full terminal applications.

```rust
smol::block_on(element!(App).fullscreen()).unwrap();
```

Fullscreen mode is appropriate when the UI owns the terminal viewport and should handle layout based on terminal size.

### Render loop configuration

`render_loop()` returns a configurable future.

```rust
smol::block_on(
    element!(App)
        .render_loop()
        .enable_mouse_capture()
        .ignore_ctrl_c(),
)?;
```

Common methods:

- `fullscreen()`
- `enable_mouse_capture()`
- `disable_mouse_capture()`
- `ignore_ctrl_c()`

## Hooks

Hooks attach behavior to components. They follow React-like rules: call hooks in the same order on every render. Do not call hooks inside conditionals, loops, or after early returns that can vary between renders.

Good:

```rust
#[component]
fn Example(mut hooks: Hooks) -> impl Into<AnyElement<'static>> {
    let enabled = hooks.use_state(|| false);
    let count = hooks.use_state(|| 0);

    if enabled.get() {
        element! { Text(content: format!("Count: {count}")) }
    } else {
        element! { Text(content: "Disabled") }
    }
}
```

Avoid:

```rust
#[component]
fn Example(mut hooks: Hooks) -> impl Into<AnyElement<'static>> {
    if feature_enabled() {
        let count = hooks.use_state(|| 0);
        element! { Text(content: format!("Count: {count}")) }
    } else {
        element! { Text(content: "Disabled") }
    }
}
```

### `use_state`

Use `use_state` for values that affect rendering. Updating state triggers a rerender.

```rust
let mut count = hooks.use_state(|| 0);
count += 1;
count.set(42);
let value = count.get();
```

`State<T>` is copyable. For copy types, `get()` returns the current value. For non-copy types, use `to_string`, `read`, or controlled setters depending on the need.

### `use_ref`

Use `use_ref` for mutable component-owned values that should not trigger rerenders.

Good uses:

- Imperative handles.
- Cached counters not shown in UI.
- Mutable bookkeeping for event handlers.

### `use_const`

Use `use_const` for values initialized once and never changed.

### `use_memo`

Use `use_memo` for expensive derived values.

```rust
let visible_rows = hooks.use_memo(
    || filter_rows(props.rows, props.query.as_deref().unwrap_or_default()),
    (props.rows.len(), props.query.clone()),
);
```

Dependencies are detected by `Hash`. Put multiple dependencies in a tuple. The memoized value is cloned when returned, so use cheap-to-clone values or reference-counted values for large data.

### `use_effect`

Use `use_effect` to run a synchronous side effect after an update pass when dependencies change.

```rust
hooks.use_effect(
    move || log::debug!("selection changed"),
    selected_id,
);
```

Use `()` as the dependency to run once after the first pass.

### `use_future`

Use `use_future` for async tasks tied to component lifetime. The future is spawned once; later calls at the same hook position have no effect.

```rust
#[component]
fn Clock(mut hooks: Hooks) -> impl Into<AnyElement<'static>> {
    let mut ticks = hooks.use_state(|| 0);

    hooks.use_future(async move {
        loop {
            smol::Timer::after(std::time::Duration::from_secs(1)).await;
            ticks += 1;
        }
    });

    element! { Text(content: format!("Ticks: {ticks}")) }
}
```

### `use_terminal_events`

Use `use_terminal_events` for keyboard and terminal-wide mouse input.

```rust
let mut should_exit = hooks.use_state(|| false);

hooks.use_terminal_events(move |event| match event {
    TerminalEvent::Key(KeyEvent { code, kind, .. }) if kind != KeyEventKind::Release => {
        match code {
            KeyCode::Char('q') => should_exit.set(true),
            _ => {}
        }
    }
    _ => {}
});
```

Ignore `KeyEventKind::Release` for most keyboard actions so a press is handled once for press/repeat behavior rather than also on release.

### `use_local_terminal_events`

Use `use_local_terminal_events` when a component should only receive terminal events occurring within its own rendered region. Mouse coordinates are translated to component-local coordinates.

### `use_terminal_size`

Use `use_terminal_size` for fullscreen or responsive layouts.

```rust
let (width, height) = hooks.use_terminal_size();

element! {
    View(width, height) {
        Text(content: "Fullscreen")
    }
}
```

### `use_context` and `use_context_mut`

Use context for cross-cutting state or system control. The system context is always available.

```rust
let mut system = hooks.use_context_mut::<SystemContext>();

if should_exit.get() {
    system.exit();
}
```

## Terminal events and exit behavior

For interactive components, keep event handling and exit behavior explicit.

Typical pattern:

```rust
#[component]
fn App(mut hooks: Hooks) -> impl Into<AnyElement<'static>> {
    let mut system = hooks.use_context_mut::<SystemContext>();
    let mut should_exit = hooks.use_state(|| false);

    hooks.use_terminal_events(move |event| match event {
        TerminalEvent::Key(KeyEvent { code, kind, .. }) if kind != KeyEventKind::Release => {
            if code == KeyCode::Char('q') {
                should_exit.set(true);
            }
        }
        _ => {}
    });

    if should_exit.get() {
        system.exit();
    }

    element! {
        Text(content: "Press q to quit")
    }
}
```

Use `SystemContext::exit()` rather than relying on external task cancellation for normal UI shutdown.

Use `SystemContext::set_mouse_capture(enabled)` when the component needs to toggle mouse capture during a render loop.

## Forms and focus management

Model focus explicitly. A common pattern is an integer state where each focusable control gets an index.

```rust
#[component]
fn Form(mut hooks: Hooks) -> impl Into<AnyElement<'static>> {
    let mut focus = hooks.use_state(|| 0usize);
    let first_name = hooks.use_state(|| String::new());
    let last_name = hooks.use_state(|| String::new());
    let mut submitted = hooks.use_state(|| false);

    hooks.use_terminal_events(move |event| match event {
        TerminalEvent::Key(KeyEvent { code, kind, .. }) if kind != KeyEventKind::Release => {
            match code {
                KeyCode::Tab => focus.set((focus.get() + 1) % 3),
                KeyCode::BackTab => focus.set((focus.get() + 2) % 3),
                KeyCode::Enter if focus.get() == 2 => submitted.set(true),
                _ => {}
            }
        }
        _ => {}
    });

    element! {
        View(flex_direction: FlexDirection::Column) {
            FormField(label: "First", value: first_name, has_focus: focus == 0)
            FormField(label: "Last", value: last_name, has_focus: focus == 1)
            Button(has_focus: focus == 2, handler: move |_| submitted.set(true)) {
                Text(content: "Submit")
            }
        }
    }
}
```

For reusable form fields, pass the field state and `has_focus` into a child component.

```rust
#[derive(Default, Props)]
struct FormFieldProps {
    label: String,
    value: Option<State<String>>,
    has_focus: bool,
    multiline: bool,
}

#[component]
fn FormField(props: &FormFieldProps) -> impl Into<AnyElement<'static>> {
    let Some(mut value) = props.value else {
        panic!("value is required");
    };

    element! {
        View(
            border_style: if props.has_focus { BorderStyle::Round } else { BorderStyle::None },
            border_color: Color::Blue,
            padding: if props.has_focus { 0 } else { 1 },
        ) {
            View(width: 15) {
                Text(content: format!("{}: ", props.label))
            }
            View(
                width: 30,
                height: if props.multiline { 5 } else { 1 },
                background_color: Color::DarkGrey,
            ) {
                TextInput(
                    value: value.to_string(),
                    has_focus: props.has_focus,
                    multiline: props.multiline,
                    on_change: move |new_value| value.set(new_value),
                )
            }
        }
    }
}
```

## Context

Use `ContextProvider` to make shared data available to descendants.

```rust
struct AppConfig {
    title: String,
}

#[component]
fn Header(hooks: Hooks) -> impl Into<AnyElement<'static>> {
    let config = hooks.use_context::<AppConfig>();

    element! {
        Text(content: config.title.clone(), weight: Weight::Bold)
    }
}

fn main() {
    element! {
        ContextProvider(value: Context::owned(AppConfig {
            title: "Dashboard".to_string(),
        })) {
            Header
        }
    }
    .print();
}
```

Context forms:

- `Context::owned(value)` for owned values.
- `Context::from_ref(&value)` for immutable borrowed values.
- `Context::from_mut(&mut value)` for mutable borrowed values.

Use context for true shared dependencies or system-level concerns. Prefer explicit props for local parent-child data.

## Tables and repeated layouts

Build tables with `View` rows and percentage widths.

```rust
#[derive(Default, Props)]
struct UsersTableProps<'a> {
    users: Option<&'a [User]>,
}

#[component]
fn UsersTable<'a>(props: &UsersTableProps<'a>) -> impl Into<AnyElement<'a>> {
    element! {
        View(
            flex_direction: FlexDirection::Column,
            width: 80,
            border_style: BorderStyle::Round,
            border_color: Color::Cyan,
        ) {
            View(border_style: BorderStyle::Single, border_edges: Edges::Bottom) {
                View(width: 20pct) { Text(content: "ID", weight: Weight::Bold) }
                View(width: 40pct) { Text(content: "Name", weight: Weight::Bold) }
                View(width: 40pct) { Text(content: "Email", weight: Weight::Bold) }
            }
            #(props.users.into_iter().flatten().enumerate().map(|(index, user)| element! {
                View(key: user.id, background_color: if index % 2 == 0 { None } else { Some(Color::DarkGrey) }) {
                    View(width: 20pct) { Text(content: user.id.to_string()) }
                    View(width: 40pct) { Text(content: user.name.clone()) }
                    View(width: 40pct) { Text(content: user.email.clone()) }
                }
            }))
        }
    }
}
```

Best practices for tables:

- Use a header row with bottom border.
- Use fixed or percentage widths deliberately.
- Use row striping only when it improves readability.
- Use stable row keys.
- Keep table components display-focused; sort/filter data before passing it in unless the table owns that interaction.

## Progress and live status

Use `use_future` plus state for progress or periodic updates.

```rust
#[component]
fn ProgressBar(mut hooks: Hooks) -> impl Into<AnyElement<'static>> {
    let mut system = hooks.use_context_mut::<SystemContext>();
    let mut progress = hooks.use_state(|| 0.0f32);

    hooks.use_future(async move {
        loop {
            smol::Timer::after(std::time::Duration::from_millis(100)).await;
            progress.set((progress.get() + 2.0).min(100.0));
        }
    });

    if progress >= 100.0 {
        system.exit();
    }

    element! {
        View {
            View(border_style: BorderStyle::Round, border_color: Color::Blue, width: 60) {
                View(width: Percent(progress.get()), height: 1, background_color: Color::Green)
            }
            View(padding_left: 1) {
                Text(content: format!("{:.0}%", progress))
            }
        }
    }
}
```

Prefer render-loop progress UI for foreground tasks. For background logs or CI output, use static or line-oriented output instead.

## Fullscreen apps

Fullscreen apps should use terminal size and provide clear exit instructions.

```rust
#[component]
fn FullscreenApp(mut hooks: Hooks) -> impl Into<AnyElement<'static>> {
    let (width, height) = hooks.use_terminal_size();
    let mut system = hooks.use_context_mut::<SystemContext>();
    let mut should_exit = hooks.use_state(|| false);

    hooks.use_terminal_events(move |event| match event {
        TerminalEvent::Key(KeyEvent { code: KeyCode::Char('q'), kind, .. })
            if kind != KeyEventKind::Release =>
        {
            should_exit.set(true);
        }
        _ => {}
    });

    if should_exit.get() {
        system.exit();
    }

    element! {
        View(
            width,
            height,
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            border_style: BorderStyle::Double,
            border_color: Color::Blue,
        ) {
            Text(content: "Fullscreen App", weight: Weight::Bold)
            Text(content: "Press q to quit", color: Color::Grey)
        }
    }
}
```

Fullscreen best practices:

- Use `use_terminal_size` and size the root `View` to `(width, height)`.
- Provide a visible quit shortcut.
- Use raw-mode keyboard handling intentionally.
- Use mouse capture only when needed.
- Keep layout resilient to small terminal sizes.

## Output and logging from components

Use `use_output` when a component must append output above the rendered UI. This is preferable to writing directly to stdout/stderr inside a render loop because IOCraft can coordinate the output with the UI.

Use ordinary logging or printing outside IOCraft for post-render summaries.

## Testing

Use static rendering tests for deterministic UI output.

```rust
#[test]
fn renders_status() {
    let actual = element! {
        Text(content: "ok", color: Color::Green)
    }
    .to_string();

    assert!(actual.contains("ok"));
}
```

For interactive behavior, use `mock_terminal_render_loop` with synthetic events.

```rust
use futures::StreamExt;
use iocraft::prelude::*;

async fn renders_typed_input() {
    let actual = element!(MyTextInput)
        .mock_terminal_render_loop(MockTerminalConfig::with_events(futures::stream::iter(vec![
            TerminalEvent::Key(KeyEvent::new(KeyEventKind::Press, KeyCode::Char('f'))),
            TerminalEvent::Key(KeyEvent::new(KeyEventKind::Release, KeyCode::Char('f'))),
            TerminalEvent::Key(KeyEvent::new(KeyEventKind::Press, KeyCode::Char('o'))),
            TerminalEvent::Key(KeyEvent::new(KeyEventKind::Release, KeyCode::Char('o'))),
        ])))
        .map(|canvas| canvas.to_string())
        .collect::<Vec<_>>()
        .await;

    assert!(actual.iter().any(|frame| frame.contains("fo")));
}
```

Test strategy:

- Snapshot or assert stable static output for static components.
- Simulate key events for focus, form, and input flows.
- Test exit behavior for render loops when practical.
- Keep tests focused on user-visible behavior, not internal hook details.

## Common mistakes

### Calling hooks conditionally

This can panic because hook order changes. Call all hooks before conditional rendering.

### Using state for values that do not affect rendering

Use `use_ref` for mutable data that should not cause rerenders.

### Missing keys in dynamic lists

Missing keys can cause child state to stick to the wrong row after insertion, deletion, or reordering.

### Rendering dynamic UI to redirected output

`render_loop` and `fullscreen` expect a TTY. If stdout may be redirected, use static rendering or detect terminal capability before choosing an interactive mode.

### Overusing custom drawing

Most UIs can be built with `View`, `Text`, `MixedText`, `ScrollView`, `TextInput`, and `Button`. Custom low-level component drawing should be a last resort.

### Letting event handlers do too much

Event handlers should usually update state. Let rendering derive UI from state. This keeps behavior predictable and easier to test.

## Implementation checklist

When implementing an IOCraft feature, verify:

- The root rendering mode matches the use case.
- Spectacular TUI transcript/status/prompt rows are composed from IOCraft components rather than new `Vec<RenderLine>`/`Vec<String>` formatters.
- Components have clear names and boundaries.
- Props borrow domain data when practical.
- Required props are validated early.
- Hooks are unconditional and ordered consistently.
- Dynamic lists use stable keys.
- Keyboard events ignore releases unless release handling is intentional.
- Interactive UIs have a clear exit path.
- Fullscreen UIs handle terminal size.
- Tests cover important static output or interactive flows.
