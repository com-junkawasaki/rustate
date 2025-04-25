#![allow(dead_code)] // Allow dead code for now during refactoring

use crate::context::Context;
use crate::error::{self as CrateErrorModule, Result, StateError};
use crate::event::{Event, EventTrait};
use crate::state::{State, StateTrait, StateType};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::fmt::{self, Debug};
use std::marker::PhantomData;
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot, RwLock};
use tokio::task::JoinHandle;
use tokio::time::Duration;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

// --- Actor Options ---
#[derive(Debug, Clone)]
pub struct ActorOptions<I: Send + Sync + 'static> {
    pub id: Option<String>,
    pub input: Option<I>,
}

// --- Snapshot ---

/// Represents the immutable state of an actor at a specific point in time.
/// Generic over the actor's context type `TContext`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Snapshot<TContext, TOutput = ()> {
    /// The current state value (e.g., hierarchical state identifier).
    /// Using serde_json::Value for flexibility, similar to how XState represents state values.
    /// Consider a more specific enum or struct representation based on machine definition later.
    pub value: serde_json::Value,
    /// The current context (extended state) of the actor.
    pub context: TContext,
    /// The output value produced when the actor reaches a final state.
    pub output: Option<TOutput>,
    /// The status of the actor (e.g., Active, Done, Stopped).
    pub status: ActorStatus,
    // Potential future additions: historyValue, error, etc.
}

impl<TContext, TOutput> Snapshot<TContext, TOutput> {
    pub fn new(
        value: serde_json::Value,
        context: TContext,
        output: Option<TOutput>,
        status: ActorStatus,
    ) -> Self {
        Self {
            value,
            context,
            output,
            status,
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActorStatus {
    Active,
    Done,
    Stopped,
    Error,
}

// --- Actor Logic Trait ---
#[async_trait]
pub trait ActorLogic<S, E, I, Q, Resp>: Send + Sync
where
    S: StateTrait + Send + Sync + 'static,
    E: EventTrait + Send + Sync + 'static,
    I: Send + Sync + 'static,
    Q: Send + Sync + 'static,
    Resp: Send + Sync + 'static,
{
    async fn handle_event(
        &self,
        state: &mut S,
        context: &mut Context,
        event: &E,
    ) -> Result<(), StateError>;

    async fn handle_query(&self, state: &S, context: &Context, query: Q) -> Resp;

    async fn decide(
        &self,
        state: &S,
        context: &Context,
        snapshot: &Option<Snapshot<Context>>,
    ) -> Result<Vec<E>, StateError>;

    fn get_initial_snapshot(&self, input: Option<I>) -> S;

    async fn transition(&self, snapshot: S, event: E) -> Result<S, StateError>;
}

// --- Actor Trait (Defines the core actor behavior instance) ---
#[async_trait]
pub trait ActorTrait<E, Q, Resp>: Send + Sync
where
    E: Send + Sync + 'static,
    Q: Send + Sync + 'static,
    Resp: Send + Sync + 'static,
{
    fn id(&self) -> Uuid;
    async fn handle_event(&mut self, event: E);
    async fn handle_query(&self, query: Q, responder: oneshot::Sender<Resp>);
    async fn started(&mut self);
    async fn stopped(&mut self);
}

// Enum for commands sent to the actor channel
#[derive(fmt::Debug)]
pub enum ActorCommand<E, Q, Resp>
where
    E: EventTrait + Send + 'static,
    Q: Send + 'static,
    Resp: Send + 'static,
{
    SendEvent(E),
    Query(Q, oneshot::Sender<Resp>),
    Stop,
}

/// Trait for events that support a query/response pattern.
pub trait QueryableEvent: EventTrait {
    type Query: Send;
    type Response: Send + fmt::Debug;
}

// --- ActorRef Implementation STRUCT (The handle) ---
pub struct ActorRefImpl<E, Q, Resp>
where
    E: EventTrait + Send + 'static,
    Q: Send + 'static,
    Resp: Send + 'static,
{
    pub id: Uuid,
    pub(crate) sender: mpsc::Sender<ActorCommand<E, Q, Resp>>,
    pub status: Arc<RwLock<ActorStatus>>,
    _phantom: PhantomData<(Q, Resp)>,
}

impl<E, Q, Resp> Clone for ActorRefImpl<E, Q, Resp>
where
    E: EventTrait + Send + 'static,
    Q: Send + 'static + Clone,
    Resp: Send + 'static + Clone,
{
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            sender: self.sender.clone(),
            status: self.status.clone(),
            _phantom: PhantomData,
        }
    }
}

impl<E, Q, Resp> fmt::Debug for ActorRefImpl<E, Q, Resp>
where
    E: EventTrait + Send + 'static,
    Q: Send + 'static,
    Resp: Send + 'static,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ActorRefImpl")
            .field("id", &self.id)
            .field("sender", &"mpsc::Sender<...>") // Simplified
            .field("status", &self.status)
            .finish()
    }
}

// --- ActorRefImpl Send/Query Methods ---
impl<E, Q, Resp> ActorRefImpl<E, Q, Resp>
where
    E: EventTrait + Send + Sync + fmt::Debug + 'static,
    Q: Send + Sync + fmt::Debug + 'static,
    Resp: Send + Sync + fmt::Debug + 'static,
{
    pub async fn send_event(&self, event: E) -> Result<(), StateError> {
        self.sender
            .send(ActorCommand::SendEvent(event))
            .await
            .map_err(|e| StateError::SendError(format!("Failed to send event: {}", e)))
    }

    pub async fn query(&self, query: Q) -> Result<Resp, StateError> {
        let (tx, rx) = oneshot::channel();
        self.sender
            .send(ActorCommand::Query(query, tx))
            .await
            .map_err(|e| StateError::SendError(format!("Failed to send query: {}", e)))?;
        rx.await.map_err(|e| {
            StateError::ReceiveError(format!("Failed to receive query response: {}", e))
        })
    }

    pub async fn stop(&self) -> Result<(), StateError> {
        self.sender
            .send(ActorCommand::Stop)
            .await
            .map_err(|e| StateError::SendError(format!("Failed to send stop command: {}", e)))
    }
}

// --- Actor Implementation (The actual actor instance) ---
pub struct ActorImpl<L, S, E, I, Q, Resp>
where
    L: ActorLogic<S, E, I, Q, Resp> + Send + Sync + 'static,
    S: StateTrait + Send + Sync + 'static,
    E: EventTrait + Send + Sync + 'static,
    I: Send + Sync + 'static,
    Q: Send + Sync + 'static,
    Resp: Send + Sync + 'static,
{
    id: Uuid,
    logic: Arc<L>,
    state: S,
    context: Arc<tokio::sync::RwLock<Context>>,
    inbox: mpsc::Receiver<ActorCommand<E, Q, Resp>>,
    status: Arc<RwLock<ActorStatus>>,
    snapshot: Option<Snapshot<Context>>,
    _phantom_i: PhantomData<I>,
    _phantom_e: PhantomData<E>,
    _phantom_resp: PhantomData<Resp>,
}

// Implementation for ActorImpl using the defined ActorTrait
#[async_trait]
impl<L, S, E, I, Q, Resp> ActorTrait<E, Q, Resp> for ActorImpl<L, S, E, I, Q, Resp>
where
    L: ActorLogic<S, E, I, Q, Resp> + Send + Sync + 'static,
    S: StateTrait + Send + Sync + 'static,
    E: EventTrait + Send + Sync + 'static,
    I: Send + Sync + 'static,
    Q: Send + Sync + 'static,
    Resp: Send + Sync + 'static + Debug,
{
    fn id(&self) -> Uuid {
        self.id
    }

    /// Handles incoming events.
    async fn handle_event(&mut self, event: E) {
        // Use write().await
        let mut context_guard = self.context.write().await;
        if let Err(e) = self
            .logic
            .handle_event(&mut self.state, &mut context_guard, &event)
            .await
        {
            error!(actor_id = %self.id, error = %e, "Error handling event");
        }
    }

    /// Handles incoming queries.
    async fn handle_query(&self, query: Q, responder: oneshot::Sender<Resp>) {
        // Use read().await
        let context_guard = self.context.read().await;
        let response = self
            .logic
            .handle_query(&self.state, &context_guard, query)
            .await;
        if responder.send(response).is_err() {
            error!(actor_id = %self.id, "Failed to send query response");
        }
    }

    async fn started(&mut self) {
        info!(actor_id = %self.id, "Actor implementation started hook.");
    }

    async fn stopped(&mut self) {
        info!(actor_id = %self.id, "Actor implementation stopped hook.");
    }
}

// --- run_actor --- Remove receiver argument
pub async fn run_actor<
    TEvent: EventTrait + Send + Sync + fmt::Debug + 'static,
    Q: Send + Sync + fmt::Debug + 'static,
    Resp: Send + Sync + fmt::Debug + 'static,
>(
    mut actor: Box<dyn ActorTrait<TEvent, Q, Resp> + Send + Sync>,
    actor_ref_id: Uuid,
) {
    info!(actor_id = %actor_ref_id, "Actor started");
    actor.started().await;

    // ActorImpl holds the receiver internally now
    // The loop logic needs to be inside ActorImpl or triggered differently.
    // This run_actor function might need complete removal or redesign
    // if ActorImpl itself manages its message loop.

    // --- TEMPORARY: Assume ActorImpl exposes its receiver or run method ---
    // This part needs significant refactoring based on ActorImpl design.
    // For now, let's assume run_actor is called *on* an ActorImpl instance
    // which has access to its own inbox.
    // The current signature where run_actor takes a Box<dyn ActorTrait> and
    // *also* a receiver is problematic.

    // --- Placeholder Loop (Likely incorrect structure) ---
    // This simulates the old loop but won't work as `actor` doesn't own the receiver.
    /*
    let mut internal_receiver = actor.get_receiver(); // Hypothetical method
    while let Some(command) = internal_receiver.recv().await {
        match command {
            ActorCommand::SendEvent(event) => {
                debug!(actor_id = %actor_ref_id, event = ?event, "Received event");
                actor.handle_event(event).await;
            }
            ActorCommand::Query(query, responder) => {
                debug!(actor_id = %actor_ref_id, query = ?query, "Received query");
                actor.handle_query(query, responder).await;
            }
            ActorCommand::Stop => {
                info!(actor_id = %actor_ref_id, "Stopping actor");
                break; // Exit the loop
            }
        }
    }
    */
    // Since the loop cannot run here with the current signature,
    // we just log finish. The actual loop needs to be part of ActorImpl.
    warn!(actor_id = %actor_ref_id, "run_actor loop logic needs refactoring within ActorImpl");

    actor.stopped().await;
    info!(actor_id = %actor_ref_id, "Actor stopped");
}

// --- create_actor --- Remove receiver from run_actor call
pub fn create_actor<L, S, E, I, Q, R>(
    logic: L,
    initial_state: S,
    context: Context,
    actor_id: Option<Uuid>,
    buffer_size: usize,
) -> (ActorRefImpl<E, Q, Result<R, StateError>>, JoinHandle<()>)
where
    L: ActorLogic<S, E, I, Q, Result<R, StateError>> + Send + Sync + 'static,
    S: StateTrait + Send + Sync + 'static,
    E: EventTrait + Send + Sync + 'static,
    I: Send + Sync + Debug + 'static,
    Q: Send + Sync + Debug + 'static,
    R: Send + Sync + Debug + 'static,
{
    let id = actor_id.unwrap_or_else(Uuid::new_v4);
    let (sender, receiver): (
        mpsc::Sender<ActorCommand<E, Q, Result<R, StateError>>>,
        mpsc::Receiver<ActorCommand<E, Q, Result<R, StateError>>>,
    ) = mpsc::channel(buffer_size);

    let actor_ref = ActorRefImpl {
        id,
        sender: sender.clone(),
        status: Arc::new(RwLock::new(ActorStatus::Active)),
        _phantom: PhantomData,
    };

    // ActorImpl takes ownership of the receiver
    let actor_instance = ActorImpl {
        id,
        logic: Arc::new(logic),
        state: initial_state,
        context: Arc::new(tokio::sync::RwLock::new(context)),
        inbox: receiver, // receiver is moved here
        status: actor_ref.status.clone(),
        snapshot: None,
        _phantom_i: PhantomData,
        _phantom_e: PhantomData,
        _phantom_resp: PhantomData::<Result<R, StateError>>,
    };

    // Box the actor instance.
    let mut actor_boxed: Box<dyn ActorTrait<E, Q, Result<R, StateError>> + Send + Sync> =
        Box::new(actor_instance);

    // Spawn a task that will run the actor's internal loop.
    // The run_actor function is not suitable anymore.
    // We need a method on ActorImpl or ActorTrait to start the loop.
    // Example: actor_boxed.run() or similar.
    // For now, let's spawn a task that just holds the actor.
    // The actual message processing loop needs to be implemented.

    let handle = tokio::spawn(async move {
        // This actor_boxed now owns the receiver via ActorImpl's inbox.
        // A method like actor_boxed.run_loop().await should be called here.
        warn!(actor_id = %id, "Actor task spawned, but internal message loop needs implementation within ActorImpl or ActorTrait");
        // Keep the actor alive until dropped or explicitly stopped.
        // In a real scenario, the internal loop would await messages.
        // For now, we can simulate keeping it alive, or let it exit.
        // Let's just drop it for now, signalling the task can complete.
        drop(actor_boxed);
    });

    (actor_ref, handle)
}

#[cfg(test)]
mod tests {
    use super::*; // Import items from parent module
    use crate::actor::{
        create_actor, ActorCommand, ActorImpl, ActorLogic, ActorRefImpl, ActorTrait, Snapshot,
    };
    use crate::error::StateError;
    use crate::event::EventTrait;
    use crate::state::{StateTrait, StateType};
    use crate::{Context, Event as CrateEvent}; // Removed QueryableEvent import as it wasn't found
    use serde::{Deserialize, Serialize};
    use std::fmt::{self, Debug, Display};
    use std::marker::PhantomData;
    use std::sync::Arc;
    use tokio::sync::{mpsc, oneshot};
    use tokio::task::JoinHandle;
    use tokio::time::Duration;
    use uuid::Uuid;

    // --- Test Fixtures ---
    #[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
    pub struct TestState {
        count: i32,
        name: String,
    }

    // Basic implementation for testing
    impl StateTrait for TestState {}
    impl Display for TestState {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "TestState(count: {}, name: {})", self.count, self.name)
        }
    }

    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    pub enum TestEvent {
        Increment,
        Decrement,
        SetName(String),
    }

    // Basic implementation for testing
    impl EventTrait for TestEvent {
        fn event_type(&self) -> &str {
            match self {
                TestEvent::Increment => "Increment",
                TestEvent::Decrement => "Decrement",
                TestEvent::SetName(_) => "SetName",
            }
        }
        fn payload(&self) -> Option<&serde_json::Value> {
            None
        }
        fn name(&self) -> &str {
            self.event_type()
        } // Simplified
    }

    #[derive(Debug, Clone, PartialEq, Eq)] // Removed Serialize, Deserialize if not needed
    enum TestQuery {
        GetCount,
        GetName,
    }

    // R - Success response type
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    struct TestResponse(String);

    // Test Logic Implementation - handle_query returns Result<TestResponse, StateError>
    #[derive(Debug)]
    struct TestActorLogic;

    #[async_trait]
    impl ActorLogic<TestState, TestEvent, (), TestQuery, Result<TestResponse, StateError>>
        for TestActorLogic
    {
        async fn handle_event(
            &self,
            state: &mut TestState,
            _context: &mut Context, // Context needs to be mut
            event: &TestEvent,
        ) -> Result<(), StateError> {
            match event {
                TestEvent::Increment => state.count += 1,
                TestEvent::Decrement => state.count -= 1,
                TestEvent::SetName(name) => state.name = name.clone(),
            }
            Ok(())
        }

        // handle_query now returns Resp = Result<R, StateError>
        async fn handle_query(
            &self,
            state: &TestState,
            _context: &Context,
            query: TestQuery,
        ) -> Result<TestResponse, StateError> {
            // Returns Resp directly
            match query {
                TestQuery::GetCount => Ok(TestResponse(format!("Count: {}", state.count))),
                TestQuery::GetName => Ok(TestResponse(format!("Name: {}", state.name))),
            }
        }

        async fn decide(
            &self,
            _state: &TestState,
            _context: &Context,
            _snapshot: &Option<Snapshot<Context>>,
        ) -> Result<Vec<TestEvent>, StateError> {
            Ok(vec![])
        }

        fn get_initial_snapshot(&self, _input: Option<()>) -> TestState {
            TestState {
                count: 0,
                name: "Default".to_string(),
            }
        }

        async fn transition(
            &self,
            snapshot: TestState,
            _event: TestEvent,
        ) -> Result<TestState, StateError> {
            Ok(snapshot)
        }
    }

    // --- Tests ---
    #[tokio::test]
    async fn test_actor_creation_and_initial_state() {
        let initial_state = TestState {
            count: 0,
            name: "Initial".to_string(),
        };
        // create_actor now expects the logic to return Result<R, Error>
        let (actor_ref, handle) = create_actor::<_, _, _, _, _, TestResponse>(
            TestActorLogic,
            initial_state.clone(),
            Context::new(None, None, None),
            None,
            100,
        );

        // actor_ref.query returns Result<Resp, Error> = Result<Result<R, Error>, Error>
        let response_res_outer = actor_ref.query(TestQuery::GetCount).await;
        assert!(
            response_res_outer.is_ok(),
            "Outer query result should be Ok"
        );
        let response_res_inner = response_res_outer.unwrap();
        assert!(
            response_res_inner.is_ok(),
            "Inner Resp (Result<R, Error>) should be Ok"
        );
        assert_eq!(
            response_res_inner.unwrap(),
            TestResponse("Count: 0".to_string())
        );

        let response_name_outer = actor_ref.query(TestQuery::GetName).await;
        assert!(response_name_outer.is_ok());
        let response_name_inner = response_name_outer.unwrap();
        assert!(response_name_inner.is_ok());
        assert_eq!(
            response_name_inner.unwrap(),
            TestResponse("Name: Initial".to_string())
        );

        // Ensure actor stops cleanly
        actor_ref.stop().await.expect("Failed to send stop signal");
        handle.await.expect("Actor task failed");
    }

    #[tokio::test]
    async fn test_actor_event_handling() {
        let initial_state = TestState {
            count: 5,
            name: "Event Tester".to_string(),
        };
        let (actor_ref, handle) = create_actor::<_, _, _, _, _, TestResponse>(
            TestActorLogic,
            initial_state.clone(),
            Context::new(None, None, None),
            None,
            100,
        );

        actor_ref.send_event(TestEvent::Increment).await.unwrap();
        actor_ref
            .send_event(TestEvent::SetName("Updated Name".to_string()))
            .await
            .unwrap();

        // Give time for events to process
        tokio::time::sleep(Duration::from_millis(50)).await;

        let response_res = actor_ref.query(TestQuery::GetCount).await.unwrap();
        assert_eq!(response_res.unwrap(), TestResponse("Count: 6".to_string()));

        let response_name = actor_ref.query(TestQuery::GetName).await.unwrap();
        assert_eq!(
            response_name.unwrap(),
            TestResponse("Name: Updated Name".to_string())
        );

        actor_ref.stop().await.expect("Failed to send stop signal");
        handle.await.expect("Actor task failed");
    }

    #[tokio::test]
    async fn test_actor_stop() {
        let initial_state = TestState {
            count: 0,
            name: "Stop Me".to_string(),
        };
        let (actor_ref, handle) = create_actor::<_, _, _, _, _, TestResponse>(
            TestActorLogic,
            initial_state.clone(),
            Context::new(None, None, None),
            None,
            100,
        );

        let stop_result = actor_ref.stop().await;
        assert!(stop_result.is_ok());

        // Wait for the actor task to finish
        let result = tokio::time::timeout(Duration::from_secs(1), handle).await;
        assert!(result.is_ok(), "Actor task timed out after stop");
        assert!(result.unwrap().is_ok(), "Actor task panicked");
    }
}
