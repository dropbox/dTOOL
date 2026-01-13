//! Elm Architecture support for structured application development.
//!
//! This module provides an implementation of [The Elm Architecture](https://guide.elm-lang.org/architecture/)
//! pattern, which organizes applications around three concepts:
//!
//! - **Model**: The application state
//! - **Update**: A function that handles messages and updates the model
//! - **View**: A function that renders the model to UI
//!
//! # Example
//!
//! ```rust,ignore
//! use inky::elm::{Cmd, ElmApp};
//! use inky::prelude::*;
//!
//! // 1. Define your model (application state)
//! #[derive(Default)]
//! struct Model {
//!     count: i32,
//! }
//!
//! // 2. Define messages (events that can happen)
//! enum Msg {
//!     Increment,
//!     Decrement,
//!     Reset,
//! }
//!
//! // 3. Define the update function
//! fn update(model: &mut Model, msg: Msg) -> Cmd<Msg> {
//!     match msg {
//!         Msg::Increment => model.count += 1,
//!         Msg::Decrement => model.count -= 1,
//!         Msg::Reset => model.count = 0,
//!     }
//!     Cmd::none()
//! }
//!
//! // 4. Define the view function
//! fn view(model: &Model) -> Node {
//!     vbox![
//!         text!(format!("Count: {}", model.count)),
//!         hbox![
//!             text!("[+]"),  // Would handle click in real app
//!             text!("[-]"),
//!         ],
//!     ].into()
//! }
//!
//! // 5. Run the application
//! fn main() -> Result<()> {
//!     ElmApp::new(Model::default())
//!         .update(update)
//!         .view(view)
//!         .run()
//! }
//! ```
//!
//! # Commands
//!
//! The `Cmd` type represents side effects that should happen after an update.
//! This keeps the update function pure while still allowing async operations.
//!
//! ```rust,ignore
//! fn update(model: &mut Model, msg: Msg) -> Cmd<Msg> {
//!     match msg {
//!         Msg::FetchData => {
//!             model.loading = true;
//!             Cmd::perform(async { fetch_data().await }, Msg::DataFetched)
//!         }
//!         Msg::DataFetched(data) => {
//!             model.loading = false;
//!             model.data = data;
//!             Cmd::none()
//!         }
//!     }
//! }
//! ```
//!
//! # Subscriptions
//!
//! Subscriptions allow your application to listen to external events like
//! timers, keyboard input, or WebSocket messages.

use std::marker::PhantomData;

/// A command representing a side effect to perform after an update.
///
/// Commands are returned from the update function to trigger asynchronous
/// operations or batch multiple effects together.
///
/// # Example
///
/// ```rust
/// use inky::elm::Cmd;
///
/// enum Msg {
///     Tick,
///     DataLoaded(String),
/// }
///
/// // No side effects
/// let cmd: Cmd<Msg> = Cmd::none();
///
/// // Batch multiple commands
/// let batch: Cmd<Msg> = Cmd::batch(vec![
///     Cmd::message(Msg::Tick),
/// ]);
/// ```
#[derive(Default)]
pub struct Cmd<Msg> {
    /// The effects to perform.
    effects: Vec<Effect<Msg>>,
}

/// Internal representation of a side effect.
enum Effect<Msg> {
    /// Send a message immediately.
    Message(Msg),
    /// Run an async task.
    #[allow(dead_code)]
    Task(Box<dyn FnOnce() -> Option<Msg> + Send>),
}

impl<Msg> Cmd<Msg> {
    /// Create a command with no effects.
    ///
    /// # Example
    ///
    /// ```rust
    /// use inky::elm::Cmd;
    ///
    /// let cmd: Cmd<()> = Cmd::none();
    /// ```
    pub fn none() -> Self {
        Self {
            effects: Vec::new(),
        }
    }

    /// Create a command that sends a message.
    ///
    /// # Example
    ///
    /// ```rust
    /// use inky::elm::Cmd;
    ///
    /// enum Msg { Refresh }
    ///
    /// let cmd = Cmd::message(Msg::Refresh);
    /// ```
    pub fn message(msg: Msg) -> Self {
        Self {
            effects: vec![Effect::Message(msg)],
        }
    }

    /// Batch multiple commands together.
    ///
    /// # Example
    ///
    /// ```rust
    /// use inky::elm::Cmd;
    ///
    /// enum Msg { A, B }
    ///
    /// let cmd = Cmd::batch(vec![
    ///     Cmd::message(Msg::A),
    ///     Cmd::message(Msg::B),
    /// ]);
    /// ```
    pub fn batch(cmds: Vec<Cmd<Msg>>) -> Self {
        let effects = cmds.into_iter().flat_map(|c| c.effects).collect();
        Self { effects }
    }

    /// Map a command's message type to another type.
    ///
    /// # Example
    ///
    /// ```rust
    /// use inky::elm::Cmd;
    ///
    /// let cmd: Cmd<i32> = Cmd::message(42);
    /// let mapped: Cmd<String> = cmd.map(|n| n.to_string());
    /// ```
    pub fn map<F, NewMsg>(self, f: F) -> Cmd<NewMsg>
    where
        F: Fn(Msg) -> NewMsg + Clone + Send + 'static,
        Msg: 'static,
        NewMsg: 'static,
    {
        let effects = self
            .effects
            .into_iter()
            .map(|effect| match effect {
                Effect::Message(msg) => Effect::Message(f(msg)),
                Effect::Task(task) => {
                    let f = f.clone();
                    Effect::Task(Box::new(move || task().map(&f)))
                }
            })
            .collect();

        Cmd { effects }
    }

    /// Check if this command has no effects.
    pub fn is_empty(&self) -> bool {
        self.effects.is_empty()
    }

    /// Get the number of effects in this command.
    pub fn len(&self) -> usize {
        self.effects.len()
    }

    /// Extract any immediate messages from this command.
    ///
    /// This is used internally by the runtime to process commands.
    #[allow(dead_code)]
    pub fn take_messages(&mut self) -> Vec<Msg> {
        let mut messages = Vec::with_capacity(self.effects.len());
        let mut remaining = Vec::with_capacity(self.effects.len());

        for effect in self.effects.drain(..) {
            match effect {
                Effect::Message(msg) => messages.push(msg),
                other => remaining.push(other),
            }
        }

        self.effects = remaining;
        messages
    }
}

/// A subscription to external events.
///
/// Subscriptions define how your application listens to external events
/// like timers, keyboard input, or network messages.
///
/// # Example
///
/// ```rust
/// use inky::elm::Sub;
/// use std::time::Duration;
///
/// enum Msg {
///     Tick,
///     KeyPressed(char),
/// }
///
/// // No subscriptions
/// let sub: Sub<Msg> = Sub::none();
///
/// // Timer subscription (conceptual)
/// // let sub = Sub::interval(Duration::from_secs(1), || Msg::Tick);
/// ```
pub struct Sub<Msg> {
    _phantom: PhantomData<Msg>,
}

impl<Msg> Sub<Msg> {
    /// Create empty subscriptions.
    pub fn none() -> Self {
        Self {
            _phantom: PhantomData,
        }
    }

    /// Batch multiple subscriptions together.
    pub fn batch(_subs: Vec<Sub<Msg>>) -> Self {
        // In a full implementation, this would combine subscriptions
        Self::none()
    }
}

impl<Msg> Default for Sub<Msg> {
    fn default() -> Self {
        Self::none()
    }
}

/// Type alias for the update function.
type UpdateFn<Model, Msg> = Box<dyn Fn(&mut Model, Msg) -> Cmd<Msg>>;

/// Type alias for the view function.
type ViewFn<Model> = Box<dyn Fn(&Model) -> crate::node::Node>;

/// Type alias for the subscriptions function.
type SubsFn<Model, Msg> = Box<dyn Fn(&Model) -> Sub<Msg>>;

/// Configuration for an Elm Architecture application.
///
/// This builder configures the model, update, view, and subscriptions
/// for an Elm-style application.
///
/// # Example
///
/// ```rust,ignore
/// use inky::elm::ElmApp;
///
/// let app = ElmApp::new(Model::default())
///     .update(update)
///     .view(view)
///     .subscriptions(subscriptions);
/// ```
pub struct ElmApp<Model, Msg> {
    /// Initial model state.
    model: Model,
    /// Update function.
    update_fn: Option<UpdateFn<Model, Msg>>,
    /// View function.
    view_fn: Option<ViewFn<Model>>,
    /// Subscriptions function.
    subscriptions_fn: Option<SubsFn<Model, Msg>>,
    /// Title for the application.
    title: Option<String>,
    _phantom: PhantomData<Msg>,
}

impl<Model, Msg> ElmApp<Model, Msg>
where
    Model: 'static,
    Msg: 'static,
{
    /// Create a new Elm application with initial model.
    ///
    /// # Example
    ///
    /// ```rust
    /// use inky::elm::ElmApp;
    ///
    /// #[derive(Default)]
    /// struct Model { count: i32 }
    ///
    /// let app = ElmApp::<Model, ()>::new(Model::default());
    /// ```
    pub fn new(model: Model) -> Self {
        Self {
            model,
            update_fn: None,
            view_fn: None,
            subscriptions_fn: None,
            title: None,
            _phantom: PhantomData,
        }
    }

    /// Set the update function.
    ///
    /// The update function takes the current model and a message,
    /// mutates the model, and returns any commands to execute.
    pub fn update<F>(mut self, f: F) -> Self
    where
        F: Fn(&mut Model, Msg) -> Cmd<Msg> + 'static,
    {
        self.update_fn = Some(Box::new(f));
        self
    }

    /// Set the view function.
    ///
    /// The view function renders the current model state to a Node tree.
    pub fn view<F>(mut self, f: F) -> Self
    where
        F: Fn(&Model) -> crate::node::Node + 'static,
    {
        self.view_fn = Some(Box::new(f));
        self
    }

    /// Set the subscriptions function.
    ///
    /// The subscriptions function returns the active subscriptions
    /// based on the current model state.
    pub fn subscriptions<F>(mut self, f: F) -> Self
    where
        F: Fn(&Model) -> Sub<Msg> + 'static,
    {
        self.subscriptions_fn = Some(Box::new(f));
        self
    }

    /// Set the application title.
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Get a reference to the current model.
    pub fn model(&self) -> &Model {
        &self.model
    }

    /// Get a mutable reference to the model.
    pub fn model_mut(&mut self) -> &mut Model {
        &mut self.model
    }

    /// Send a message through the update function.
    ///
    /// Returns any commands that should be executed.
    pub fn send(&mut self, msg: Msg) -> Cmd<Msg> {
        if let Some(ref update_fn) = self.update_fn {
            update_fn(&mut self.model, msg)
        } else {
            Cmd::none()
        }
    }

    /// Render the current model to a Node tree.
    pub fn render(&self) -> crate::node::Node {
        if let Some(ref view_fn) = self.view_fn {
            view_fn(&self.model)
        } else {
            crate::node::BoxNode::new().into()
        }
    }
}

/// Helper trait for creating applications with the Elm pattern.
///
/// This trait can be implemented on your Model type for a more
/// object-oriented style.
///
/// # Example
///
/// ```rust,ignore
/// use inky::elm::{Cmd, ElmModel};
/// use inky::node::Node;
///
/// struct Counter {
///     count: i32,
/// }
///
/// enum CounterMsg {
///     Increment,
///     Decrement,
/// }
///
/// impl ElmModel for Counter {
///     type Msg = CounterMsg;
///
///     fn update(&mut self, msg: Self::Msg) -> Cmd<Self::Msg> {
///         match msg {
///             CounterMsg::Increment => self.count += 1,
///             CounterMsg::Decrement => self.count -= 1,
///         }
///         Cmd::none()
///     }
///
///     fn view(&self) -> Node {
///         // Render the counter...
///         # unimplemented!()
///     }
/// }
/// ```
pub trait ElmModel {
    /// The message type for this model.
    type Msg;

    /// Update the model in response to a message.
    fn update(&mut self, msg: Self::Msg) -> Cmd<Self::Msg>;

    /// Render the model to a Node tree.
    fn view(&self) -> crate::node::Node;

    /// Get subscriptions for this model.
    ///
    /// Override this to add subscriptions like timers.
    fn subscriptions(&self) -> Sub<Self::Msg> {
        Sub::none()
    }
}

/// Create an ElmApp from a type implementing ElmModel.
impl<M> From<M> for ElmApp<M, M::Msg>
where
    M: ElmModel + 'static,
    M::Msg: 'static,
{
    fn from(model: M) -> Self {
        ElmApp::new(model)
            .update(|m, msg| m.update(msg))
            .view(|m| m.view())
            .subscriptions(|m| m.subscriptions())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct TestModel {
        count: i32,
    }

    #[derive(Clone)]
    enum TestMsg {
        Increment,
        Decrement,
        Set(i32),
    }

    fn test_update(model: &mut TestModel, msg: TestMsg) -> Cmd<TestMsg> {
        match msg {
            TestMsg::Increment => model.count += 1,
            TestMsg::Decrement => model.count -= 1,
            TestMsg::Set(n) => model.count = n,
        }
        Cmd::none()
    }

    fn test_view(model: &TestModel) -> crate::node::Node {
        crate::node::TextNode::new(format!("Count: {}", model.count)).into()
    }

    #[test]
    fn test_cmd_none() {
        let cmd: Cmd<()> = Cmd::none();
        assert!(cmd.is_empty());
        assert_eq!(cmd.len(), 0);
    }

    #[test]
    fn test_cmd_message() {
        let mut cmd = Cmd::message(TestMsg::Increment);
        assert!(!cmd.is_empty());
        assert_eq!(cmd.len(), 1);

        let messages = cmd.take_messages();
        assert_eq!(messages.len(), 1);
    }

    #[test]
    fn test_cmd_batch() {
        let cmd = Cmd::batch(vec![
            Cmd::message(TestMsg::Increment),
            Cmd::message(TestMsg::Decrement),
            Cmd::none(),
        ]);
        assert_eq!(cmd.len(), 2);
    }

    #[test]
    fn test_cmd_map() {
        let cmd: Cmd<i32> = Cmd::message(42);
        let mapped: Cmd<String> = cmd.map(|n| n.to_string());
        assert_eq!(mapped.len(), 1);
    }

    #[test]
    fn test_elm_app_creation() {
        let app = ElmApp::<TestModel, TestMsg>::new(TestModel::default())
            .update(test_update)
            .view(test_view)
            .title("Test App");

        assert_eq!(app.model().count, 0);
    }

    #[test]
    fn test_elm_app_send() {
        let mut app = ElmApp::new(TestModel::default())
            .update(test_update)
            .view(test_view);

        app.send(TestMsg::Increment);
        assert_eq!(app.model().count, 1);

        app.send(TestMsg::Increment);
        assert_eq!(app.model().count, 2);

        app.send(TestMsg::Decrement);
        assert_eq!(app.model().count, 1);

        app.send(TestMsg::Set(100));
        assert_eq!(app.model().count, 100);
    }

    #[test]
    fn test_elm_app_render() {
        let app = ElmApp::new(TestModel { count: 42 })
            .update(test_update)
            .view(test_view);

        let node = app.render();
        assert!(matches!(
            node,
            crate::node::Node::Text(t) if t.content.contains("42")
        ));
    }

    #[test]
    fn test_sub_none() {
        let sub: Sub<()> = Sub::none();
        let _default: Sub<()> = Sub::default();
        let _batch = Sub::batch(vec![sub]);
    }

    #[test]
    fn test_elm_model_trait() {
        struct Counter {
            value: i32,
        }

        enum CounterMsg {
            Inc,
        }

        impl ElmModel for Counter {
            type Msg = CounterMsg;

            fn update(&mut self, msg: Self::Msg) -> Cmd<Self::Msg> {
                match msg {
                    CounterMsg::Inc => self.value += 1,
                }
                Cmd::none()
            }

            fn view(&self) -> crate::node::Node {
                crate::node::TextNode::new(self.value.to_string()).into()
            }
        }

        let mut app: ElmApp<Counter, CounterMsg> = Counter { value: 0 }.into();
        app.send(CounterMsg::Inc);
        assert_eq!(app.model().value, 1);
    }
}
