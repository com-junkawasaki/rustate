#![allow(dead_code)] // Allow dead code for now during refactoring

use crate::error::{Error as CrateError, Result, StateError};
use crate::event::EventTrait;
use crate::state::StateTrait;
use crate::Context;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::fmt::{self, Debug};
use std::marker::PhantomData;
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot, RwLock};
use tokio::task::JoinHandle;
use tokio::time::Duration;
use tracing::{debug, error, info};
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
pub trait ActorLogic<S, E, I, Q, R>: Send + Sync {
    async fn handle_event(
        &self,
        state: &mut S,
        context: &Context,
        event: &E,
    ) -> Result<(), CrateError>;
    async fn handle_query(&self, state: &S, context: &Context, query: Q) -> Result<R, CrateError>;
    async fn decide(
        &self,
        state: &S,
        context: &Context,
        snapshot: &Option<Snapshot<Context>>,
    ) -> Result<Vec<E>, CrateError>;
    // Placeholder methods to match Machine impl ActorLogic usage
    fn get_initial_snapshot(&self, input: Option<I>) -> S; // Assuming S is the snapshot type here
    async fn transition(&self, snapshot: S, event: E) -> Result<S, StateError>; // Assuming S is the snapshot type
}

// --- Actor Trait (Defines the core actor behavior instance) ---
#[async_trait]
pub trait ActorTrait<E, Q, Resp>: Send + Sync {
    fn id(&self) -> Uuid;
    async fn handle_event(&mut self, event: E);
    async fn handle_query(&self, query: Q) -> Resp; // Resp is Result<R, StateError>
    async fn started(&mut self);
    async fn stopped(&mut self);
}

// --- Actor Reference TRAIT (Defines the handle/interface) ---
// This trait might be unnecessary if ActorRefImpl provides the desired public interface.
// Commenting out for now to resolve E0404/E0428 errors.
/*
#[async_trait::async_trait]
pub trait ActorRefTrait<TEvent, TSnapshot>: Send + Sync + fmt::Debug
where
    TEvent: EventTrait + Send + Sync + fmt::Debug + 'static,
    TSnapshot: Clone + Send + Sync + 'static + fmt::Debug,
{
    fn send(&self, event: TEvent) -> Result<(), StateError>;
    fn id(&self) -> &str;
    fn get_snapshot(&self) -> TSnapshot;
    fn clone_ref(&self) -> Box<dyn ActorRefTrait<TEvent, TSnapshot>>;
    fn stop(&self) -> Result<(), StateError>;
    async fn query(&self, query: TEvent::Query) -> Result<TEvent::Response, StateError>
    where
        TEvent: QueryableEvent,
        TEvent::Query: Send + Sync,
        TEvent::Response: Send + Sync + fmt::Debug;
}
*/

// Enum for commands sent to the actor channel
#[derive(fmt::Debug)]
pub enum ActorCommand<E, Q, Resp>
// Removed E: EventTrait bound here, add where needed
where
    // E: EventTrait + Send + 'static,
    Q: Send + 'static,
    Resp: Send + 'static,
{
    Send(E),
    Query(Q, oneshot::Sender<Resp>), // Resp is Result<R, StateError>
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
// Removed bounds here, enforce on methods/functions using it
// E: EventTrait + Send + Sync + fmt::Debug + 'static,
// Q: Send + Sync + fmt::Debug + 'static,
// Resp: Send + Sync + fmt::Debug + 'static,
{
    pub id: Uuid,
    pub sender: mpsc::Sender<ActorCommand<E, Q, Resp>>,
    _query_marker: PhantomData<Q>,
    _response_marker: PhantomData<Resp>,
}

impl<E, Q, Resp> Clone for ActorRefImpl<E, Q, Resp> {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            sender: self.sender.clone(), // Sender is clonable
            _query_marker: PhantomData,
            _response_marker: PhantomData,
        }
    }
}

impl<E, Q, Resp> fmt::Debug for ActorRefImpl<E, Q, Resp> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ActorRefImpl")
            .field("id", &self.id)
            .finish()
    }
}

// --- ActorRefImpl Send/Query Methods ---
impl<E, Q, Resp> ActorRefImpl<E, Q, Resp>
where
    E: Send + Sync + fmt::Debug + 'static, // Add necessary bounds here
    Q: Send + Sync + fmt::Debug + 'static,
    Resp: Send + Sync + fmt::Debug + 'static, // Resp here is Result<R, StateError>
{
    pub async fn send_event(&self, event: E) -> Result<(), StateError> {
        self.sender
            .send(ActorCommand::Send(event))
            .await
            .map_err(|e| StateError::SendError(format!("Failed to send event: {}", e)))
    }

    pub async fn query(&self, query: Q) -> Result<Resp, StateError> // Return Resp (Result<R, StateError>)
    {
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
pub struct ActorImpl<L, S, E, I, Q, R>
where
    L: ActorLogic<S, E, I, Q, R> + Send + Sync + 'static,
    S: StateTrait<Context = Context, Event = E> + Send + Sync + 'static,
    E: EventTrait + Send + Sync + 'static,
    I: Send + Sync + Debug + 'static,
    Q: Send + Sync + Debug + 'static,
    R: Send + Sync + Debug + 'static,
{
    id: Uuid,
    logic: Arc<L>,
    state: S,
    context: Arc<tokio::sync::Mutex<Context>>,
    _marker: PhantomData<(I, Q, R)>,
}

// Implementation for ActorImpl using the defined ActorTrait
#[async_trait]
impl<L, S, E, I, Q, R> ActorTrait<E, Q, Result<R, StateError>> for ActorImpl<L, S, E, I, Q, R>
where
    L: ActorLogic<S, E, I, Q, R> + Send + Sync + 'static,
    S: StateTrait<Context = Context, Event = E> + Send + Sync + 'static,
    E: EventTrait + Send + Sync + 'static,
    I: Send + Sync + Debug + 'static,
    Q: Send + Sync + Debug + 'static,
    R: Send + Sync + Debug + 'static,
{
    fn id(&self) -> Uuid {
        self.id
    }

    async fn handle_event(&mut self, event: E) {
        let mut context_guard = self.context.lock().await;
        if let Err(e) = self
            .logic
            .handle_event(&mut self.state, &mut context_guard, &event)
            .await
        {
            error!(actor_id = %self.id, error = ?e, event = ?event, "Error handling event");
        } else {
            // Event handled successfully
        }
    }

    async fn handle_query(&self, query: Q) -> Result<R, StateError> {
        let context_guard = self.context.lock().await;
        match self
            .logic
            .handle_query(&self.state, &context_guard, query)
            .await
        {
            Ok(response) => Ok(response),
            Err(e) => {
                error!(actor_id = %self.id, error = ?e, "Error handling query");
                Err(StateError::QueryError(format!(
                    "Query handling failed: {:?}",
                    e
                )))
            }
        }
    }

    async fn started(&mut self) {
        info!(actor_id = %self.id, "Actor implementation started hook.");
    }

    async fn stopped(&mut self) {
        info!(actor_id = %self.id, "Actor implementation stopped hook.");
    }
}

// Run the actor main loop
pub async fn run_actor<
    TEvent: Send + Sync + fmt::Debug + 'static, // Removed EventTrait bound, add if needed by ActorCommand usage
    Q: Send + Sync + fmt::Debug + 'static,
    Resp: Send + Sync + fmt::Debug + 'static, // This is Result<R, StateError>
>(
    mut actor: Box<dyn ActorTrait<TEvent, Q, Resp> + Send + Sync>,
    mut receiver: mpsc::Receiver<ActorCommand<TEvent, Q, Resp>>, // Command uses Resp = Result<R, StateError>
    actor_ref_id: Uuid,
) {
    info!(actor_id = %actor_ref_id, "Actor started");
    actor.started().await;

    while let Some(command) = receiver.recv().await {
        match command {
            ActorCommand::Send(event) => {
                debug!(actor_id = %actor_ref_id, event = ?event, "Received event");
                actor.handle_event(event).await;
            }
            ActorCommand::Query(query, responder) => {
                debug!(actor_id = %actor_ref_id, query = ?query, "Received query");
                let response_result = actor.handle_query(query).await;
                if let Err(_) = responder.send(response_result) {
                    // Send Resp directly
                    error!(actor_id = %actor_ref_id, "Failed to send query response: receiver dropped");
                }
            }
            ActorCommand::Stop => {
                info!(actor_id = %actor_ref_id, "Stopping actor");
                break;
            }
        }
    }

    actor.stopped().await;
    info!(actor_id = %actor_ref_id, "Actor stopped");
}

// Removed _spawn_actor function

// --- create_actor function ---
// Returns the concrete ActorImpl (boxed) and ActorRefImpl for interaction
pub fn create_actor<L, S, E, I, Q, R>(
    logic: L,
    initial_state: S,
    actor_id: Option<Uuid>,
    context: Context,
) -> (
    // Removed: Box<dyn ActorTrait<E, Q, Result<R, StateError>> + Send + Sync>, // Don't return the boxed actor itself
    ActorRefImpl<E, Q, Result<R, StateError>>, // Return the handle to interact
    JoinHandle<()>,                            // The task handle
)
where
    L: ActorLogic<S, E, I, Q, R> + Send + Sync + 'static,
    S: StateTrait<Context = Context, Event = E> + Send + Sync + 'static,
    E: EventTrait + Send + Sync + 'static,
    I: Send + Sync + Debug + 'static,
    Q: Send + Sync + Debug + 'static,
    R: Send + Sync + Debug + 'static,
{
    let id = actor_id.unwrap_or_else(Uuid::new_v4);
    // Channel for Result<R, StateError>
    let (sender, receiver) = mpsc::channel::<ActorCommand<E, Q, Result<R, StateError>>>(100);

    let actor_ref = ActorRefImpl {
        id,
        sender: sender.clone(),
        _query_marker: PhantomData,
        _response_marker: PhantomData,
    };

    let actor_instance = ActorImpl::<L, S, E, I, Q, R> {
        id,
        logic: Arc::new(logic),
        state: initial_state,
        context: Arc::new(tokio::sync::Mutex::new(context)),
        _marker: PhantomData,
    };

    let actor_boxed: Box<dyn ActorTrait<E, Q, Result<R, StateError>> + Send + Sync> =
        Box::new(actor_instance);

    let handle = tokio::spawn(run_actor(actor_boxed, receiver, id));

    (actor_ref, handle) // Return the ref and the handle
}

// Removed spawn function

// Removed old Actor trait

#[cfg(test)]
mod tests {
    use super::{
        create_actor, run_actor, ActorCommand, ActorImpl, ActorLogic, ActorRefImpl, ActorTrait,
    }; // Import necessary items
    use crate::actor::Snapshot;
    use crate::error::{Error as CrateError, StateError};
    use crate::event::EventTrait;
    use crate::state::{State, StateTrait, StateType}; // Import StateTrait
    use crate::{Context, Event as CrateEvent, QueryableEvent}; // Import EventTrait
    use serde::{Deserialize, Serialize};
    use std::fmt::{self, Debug, Display};
    use std::marker::PhantomData;
    use std::sync::Arc;
    use tokio::sync::{mpsc, oneshot};
    use tokio::task::JoinHandle;
    use tokio::time::Duration;
    use uuid::Uuid; // Use crate::actor::Snapshot

    // --- Test Fixtures ---
    #[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
    pub struct TestState {
        count: i32,
        name: String,
    }

    impl StateTrait for TestState {
        type Context = Context;
        type Event = TestEvent;
        fn name(&self) -> &str {
            "TestState"
        }
        fn state_type(&self) -> StateType {
            StateType::Atomic
        }
        // Mock required methods if StateTrait requires more
        fn parent(&self) -> Option<Self> {
            None
        }
        fn initial(&self) -> Option<Self> {
            None
        }
        fn history(&self) -> Option<crate::state::HistoryType> {
            None
        }
    }

    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    pub enum TestEvent {
        Increment,
        Decrement,
        SetName(String),
    }

    #[typetag::serde]
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
            match self {
                TestEvent::Increment => "TestIncrementEvent",
                TestEvent::Decrement => "TestDecrementEvent",
                TestEvent::SetName(_) => "TestSetNameEvent",
            }
        }
        fn topic(&self) -> &str {
            "test"
        }
        fn key(&self) -> Option<String> {
            None
        }
    }

    #[derive(Debug, Clone, PartialEq)]
    enum TestQuery {
        GetCount,
        GetName,
    }

    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    struct TestResponse(String);

    impl QueryableEvent for TestEvent {
        type Query = TestQuery;
        type Response = TestResponse;
    }

    // Test Logic Implementation
    #[derive(Debug)]
    struct TestActorLogic;

    #[async_trait]
    impl ActorLogic<TestState, TestEvent, (), TestQuery, TestResponse> for TestActorLogic {
        async fn handle_event(
            &self,
            state: &mut TestState,
            _context: &Context,
            event: &TestEvent,
        ) -> Result<(), CrateError> {
            match event {
                TestEvent::Increment => state.count += 1,
                TestEvent::Decrement => state.count -= 1,
                TestEvent::SetName(name) => state.name = name.clone(),
            }
            Ok(())
        }

        async fn handle_query(
            &self,
            state: &TestState,
            _context: &Context,
            query: TestQuery,
        ) -> Result<TestResponse, CrateError> {
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
        ) -> Result<Vec<TestEvent>, CrateError> {
            Ok(vec![])
        }

        fn get_initial_snapshot(&self, _input: Option<()>) -> TestState {
            // This method doesn't seem right for ActorLogic, state is passed in handle_event/query
            // Returning a default state, but the logic should likely not own the state creation.
            TestState {
                count: 0,
                name: "DefaultInitial".to_string(),
            }
        }

        async fn transition(
            &self,
            snapshot: TestState,
            _event: TestEvent,
        ) -> Result<TestState, StateError> {
            // Placeholder transition logic
            Ok(snapshot)
        }
    }

    // --- Tests --- Use create_actor now
    #[tokio::test]
    async fn test_actor_creation_and_initial_state() {
        let initial_state = TestState {
            count: 0,
            name: "Initial".to_string(),
        };
        let (actor_ref, handle) = create_actor(
            TestActorLogic,
            initial_state.clone(),
            None,
            Context::new(None, None, None),
        );

        // Use actor_ref (ActorRefImpl) for queries
        let response_res = actor_ref.query(TestQuery::GetCount).await;
        assert!(response_res.is_ok());
        let inner_res = response_res.unwrap();
        assert!(inner_res.is_ok());
        assert_eq!(inner_res.unwrap(), TestResponse("Count: 0".to_string()));

        let response_name_res = actor_ref.query(TestQuery::GetName).await;
        assert!(response_name_res.is_ok());
        let inner_name_res = response_name_res.unwrap();
        assert!(inner_name_res.is_ok());
        assert_eq!(
            inner_name_res.unwrap(),
            TestResponse("Name: Initial".to_string())
        );

        drop(actor_ref);
        let _ = tokio::time::timeout(Duration::from_secs(1), handle).await;
    }

    #[tokio::test]
    async fn test_actor_event_handling() {
        let initial_state = TestState {
            count: 5,
            name: "Event Tester".to_string(),
        };
        let (actor_ref, handle) = create_actor(
            TestActorLogic,
            initial_state.clone(),
            None,
            Context::new(None, None, None),
        );

        actor_ref.send_event(TestEvent::Increment).await.unwrap();
        actor_ref
            .send_event(TestEvent::SetName("Updated Name".to_string()))
            .await
            .unwrap();

        tokio::time::sleep(Duration::from_millis(50)).await;

        let response_res = actor_ref.query(TestQuery::GetCount).await;
        assert!(response_res.is_ok());
        let inner_res = response_res.unwrap();
        assert!(inner_res.is_ok());
        assert_eq!(inner_res.unwrap(), TestResponse("Count: 6".to_string()));

        let response_name_res = actor_ref.query(TestQuery::GetName).await;
        assert!(response_name_res.is_ok());
        let inner_name_res = response_name_res.unwrap();
        assert!(inner_name_res.is_ok());
        assert_eq!(
            inner_name_res.unwrap(),
            TestResponse("Name: Updated Name".to_string())
        );

        drop(actor_ref);
        let _ = tokio::time::timeout(Duration::from_secs(1), handle).await;
    }

    #[tokio::test]
    async fn test_actor_query() {
        let initial_state = TestState {
            count: 10,
            name: "Query".to_string(),
        };
        let (actor_ref, handle) = create_actor(
            TestActorLogic,
            initial_state.clone(),
            None,
            Context::new(None, None, None),
        );

        let result = actor_ref.query(TestQuery::GetCount).await;
        assert!(result.is_ok());
        let response_result = result.unwrap();
        assert!(response_result.is_ok());
        let response = response_result.unwrap();
        assert_eq!(response, TestResponse("Count: 10".to_string()));

        drop(actor_ref);
        let _ = tokio::time::timeout(Duration::from_secs(1), handle).await;
    }

    #[tokio::test]
    async fn test_actor_stop() {
        let initial_state = TestState {
            count: 0,
            name: "Stop Me".to_string(),
        };
        let (actor_ref, handle) = create_actor(
            TestActorLogic,
            initial_state.clone(),
            None,
            Context::new(None, None, None),
        );

        let stop_result = actor_ref.stop().await;
        assert!(stop_result.is_ok());

        let result = tokio::time::timeout(Duration::from_secs(2), handle).await;
        assert!(
            result.is_ok(),
            "Actor task did not complete within timeout after stop"
        );
        assert!(result.unwrap().is_ok(), "Actor task panicked");
    }
}
