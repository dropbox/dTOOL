# Async App Loop

Enable the `async` feature to use `AsyncApp` with Tokio:

```toml
inky = { version = "0.1", features = ["async"] }
```

## Basic Usage

```rust,no_run
use inky::app::AsyncApp;
use inky::prelude::*;

#[derive(Clone, Default)]
struct AppState {
    text: String,
}

#[derive(Clone)]
enum Msg {
    Append(String),
    Quit,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let app = AsyncApp::new()
        .state(AppState::default())
        .message_type::<Msg>()
        .render(|ctx| TextNode::new(&ctx.state.text).into())
        .on_message(|state, msg| {
            match msg {
                Msg::Append(text) => state.text.push_str(&text),
                Msg::Quit => return true,
            }
            false
        });

    let handle = app.async_handle();
    tokio::spawn(async move {
        handle.send(Msg::Append("Hello".into()));
        handle.render();
    });

    app.run_async().await
}
```

## AsyncAppHandle

The `AsyncAppHandle` allows background tasks to communicate with the UI:

| Method | Description |
|--------|-------------|
| `send(msg)` | Send a message to the app |
| `render()` | Request a re-render |

This enables patterns like:
- Streaming API responses
- Background file operations
- Timer-based updates
- WebSocket event handling

## Example: Streaming Output

```rust,no_run
use inky::app::AsyncApp;
use inky::prelude::*;

#[derive(Clone, Default)]
struct State {
    output: String,
}

#[derive(Clone)]
enum Msg {
    AppendToken(String),
}

async fn stream_tokens(handle: AsyncAppHandle<State, Msg>) {
    let tokens = vec!["Hello", " ", "world", "!"];
    for token in tokens {
        handle.send(Msg::AppendToken(token.into()));
        handle.render();
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let app = AsyncApp::new()
        .state(State::default())
        .message_type::<Msg>()
        .render(|ctx| TextNode::new(&ctx.state.output).into())
        .on_message(|state, msg| {
            match msg {
                Msg::AppendToken(t) => state.output.push_str(&t),
            }
            false
        });

    let handle = app.async_handle();
    tokio::spawn(stream_tokens(handle));

    app.run_async().await
}
```
