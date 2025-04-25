#![allow(dead_code)] // Allow dead code for now during refactoring

use crate::error::{Result, StateError};
use crate::event::EventTrait;
use crate::state::StateTrait;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::fmt::{self, Debug};
use std::marker::PhantomData;
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot, RwLock};
use tokio::task::JoinHandle;
use tracing::{info, warn};
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
pub trait ActorLogic<S, C, E, I, Q, Resp>: Send + Sync
where
    S: StateTrait + Send + Sync + 'static,
    C: Send + Sync + 'static + Default + Clone + Debug,
    E: EventTrait + Send + Sync + 'static,
    I: Send + Sync + 'static,
    Q: Send + Sync + 'static,
    Resp: Send + Sync + 'static,
{
    fn initial(&self) -> (S, C);

    async fn transition(&self, state: S, context: C, event: E) -> Result<(S, C), StateError>;

    async fn handle_query(&self, state: &S, context: &C, query: Q) -> Resp;
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
    async fn handle_event(&mut self, event: E) -> Result<(), StateError>;
    async fn handle_query(&self, query: Q, responder: oneshot::Sender<Resp>);
    async fn started(&mut self);
    async fn stopped(&mut self);
}

// Enum for commands sent to the actor channel
#[derive(fmt::Debug)]
pub enum ActorCommand<E, Q, Resp, C, R>
where
    E: EventTrait + Send + 'static,
    Q: Send + 'static,
    Resp: Send + 'static,
    C: Send + 'static,
    R: Send + 'static,
{
    SendEvent(E),
    Query(Q, oneshot::Sender<Resp>),
    GetSnapshot(oneshot::Sender<Result<Snapshot<C, R>, StateError>>),
    Stop,
}

/// Trait for events that support a query/response pattern.
pub trait QueryableEvent: EventTrait {
    type Query: Send;
    type Response: Send + fmt::Debug;
}

// --- ActorRef Implementation STRUCT (The handle) ---
pub struct ActorRefImpl<E, Q, Resp, C, R>
where
    E: EventTrait + Send + 'static,
    Q: Send + 'static,
    Resp: Send + 'static,
    C: Send + 'static,
    R: Send + 'static,
{
    pub id: Uuid,
    pub(crate) sender: mpsc::Sender<ActorCommand<E, Q, Resp, C, R>>,
    pub status: Arc<RwLock<ActorStatus>>,
    _phantom: PhantomData<(Q, Resp, C, R)>,
}

impl<E, Q, Resp, C, R> Clone for ActorRefImpl<E, Q, Resp, C, R>
where
    E: EventTrait + Send + 'static,
    Q: Send + 'static + Clone,
    Resp: Send + 'static + Clone,
    C: Send + 'static,
    R: Send + 'static,
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

impl<E, Q, Resp, C, R> fmt::Debug for ActorRefImpl<E, Q, Resp, C, R>
where
    E: EventTrait + Send + 'static,
    Q: Send + 'static,
    Resp: Send + 'static,
    C: Send + 'static,
    R: Send + 'static,
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
impl<E, Q, Resp, C, R> ActorRefImpl<E, Q, Resp, C, R>
where
    E: EventTrait + Send + Sync + fmt::Debug + 'static,
    Q: Send + Sync + fmt::Debug + 'static,
    Resp: Send + Sync + fmt::Debug + 'static,
    C: Send + Sync + 'static,
    R: Send + Sync + 'static,
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

// Define the internal state struct
struct InternalActorState<L, C, E, S, I, Q, R, Resp>
where
    L: ActorLogic<S, C, E, I, Q, Resp> + Send + Sync + 'static,
    C: Send + Sync + 'static + Default + Clone + Debug,
    S: StateTrait + Send + Sync + 'static,
    E: EventTrait + Send + Sync + 'static,
    I: Send + Sync + 'static,
    Q: Send + Sync + 'static,
    R: Send + Sync + 'static + Debug,
    Resp: Send + Sync + 'static + Debug,
{
    logic: Arc<L>, // Store Arc<L>
    state: S,
    context: Arc<RwLock<C>>,
    inbox: mpsc::Receiver<ActorCommand<E, Q, Resp, C, R>>,
    status: Arc<RwLock<ActorStatus>>,
    // Add necessary PhantomData if not all generics are used directly in fields
    _phantom_i: PhantomData<I>,
    _phantom_r: PhantomData<R>,
}

// --- Actor Implementation (The actual actor instance) ---
pub struct ActorImpl<L, C, E, S, I, Q, R, Resp>
where
    L: ActorLogic<S, C, E, I, Q, Resp> + Send + Sync + 'static,
    C: Send + Sync + 'static + Default + Clone + Debug,
    S: StateTrait + Send + Sync + 'static,
    E: EventTrait + Send + Sync + 'static,
    I: Send + Sync + 'static,
    Q: Send + Sync + 'static,
    R: Send + Sync + 'static + Debug,
    Resp: Send + Sync + 'static + Debug,
{
    id: Uuid,
    logic: Arc<L>,
    initial_state: S,
    context: C, // Context passed in, wrapped later
    actor_id: Option<Uuid>,
    buffer_size: usize,
    // Use correct generics for ActorCommand here
    inbox: mpsc::Receiver<ActorCommand<E, Q, Resp, C, R>>,
    status: Arc<RwLock<ActorStatus>>,
    snapshot: Option<Snapshot<C, R>>, // Adjusted Snapshot generics
    // Phantom data
    _phantom_l: PhantomData<L>,
    _phantom_e: PhantomData<E>,
    _phantom_s: PhantomData<S>,
    _phantom_i: PhantomData<I>,
    _phantom_q: PhantomData<Q>,
    _phantom_r: PhantomData<R>,
    _phantom_resp: PhantomData<Resp>,
}

// Implementation for ActorImpl using the defined ActorTrait
#[async_trait]
impl<L, C, E, S, I, Q, R, Resp> ActorTrait<E, Q, Resp> for ActorImpl<L, C, E, S, I, Q, R, Resp>
where
    L: ActorLogic<S, C, E, I, Q, Resp> + Send + Sync + 'static,
    S: StateTrait + Send + Sync + 'static,
    C: Send + Sync + 'static + Default + Clone + Debug,
    E: EventTrait + Send + Sync + 'static,
    I: Send + Sync + 'static + Default,
    Q: Send + Sync + 'static,
    R: Send + Sync + 'static + Debug + Default,
    Resp: Send + Sync + 'static + Debug,
{
    fn id(&self) -> Uuid {
        self.id
    }

    async fn handle_event(&mut self, _event: E) -> Result<(), StateError> {
        log::warn!("handle_event called directly - deprecated actor_id={}", self.id);
        Ok(())
    }

    async fn handle_query(&self, _query: Q, responder: oneshot::Sender<Resp>) {
        log::warn!("handle_query called directly - deprecated actor_id={}", self.id);
        drop(responder);
    }

    async fn started(&mut self) {
        log::debug!("ActorTrait started hook. actor_id={}", self.id);
    }

    async fn stopped(&mut self) {
        log::debug!("ActorTrait stopped hook. actor_id={}", self.id);
    }
}

impl<L, C, E, S, I, Q, R, Resp> ActorImpl<L, C, E, S, I, Q, R, Resp>
where
    L: ActorLogic<S, C, E, I, Q, Resp> + Send + Sync + 'static,
    C: Send + Sync + 'static + Default + Clone + Debug,
    S: StateTrait + Send + Sync + 'static + PartialEq,
    E: EventTrait + Send + Sync + 'static,
    I: Send + Sync + 'static + Default,
    Q: Send + Sync + 'static,
    R: Send + Sync + 'static + Debug + Default,
    Resp: Send + Sync + 'static + Debug,
{
    pub async fn run(self) { // Take self ownership
        let (initial_state, initial_context) = self.logic.initial();
        let mut actor_state = InternalActorState {
            logic: self.logic, // Arc<L>
            state: initial_state,
            context: Arc::new(RwLock::new(initial_context)), // Wrap context
            // Use correct generics for ActorCommand here
            inbox: self.inbox, // inbox was moved from self
            status: self.status.clone(),
            _phantom_i: PhantomData,
            _phantom_r: PhantomData,
        };
        let actor_id = self.id; // Capture id before self is consumed by loop
        log::info!("Actor started actor_id={}", actor_id);

        loop {
             tokio::select! {
                Some(command) = actor_state.inbox.recv() => {
                    match command {
                        ActorCommand::SendEvent(event) => {
                            let event_clone = event.clone(); // Clone event for potential reuse
                            let mut context_guard = actor_state.context.write().await;
                            // Call corrected transition signature
                            match actor_state.logic.transition(actor_state.state.clone(), (*context_guard).clone(), event_clone).await {
                                Ok((next_state, next_context)) => {
                                    if actor_state.state != next_state {
                                        log::debug!("State transition: {:?} -> {:?} actor_id={}", actor_state.state, next_state, actor_id);
                                        actor_state.state = next_state;
                                    }
                                    *context_guard = next_context; // Update context
                                }
                                Err(e) => {
                                     log::error!("Error processing event: {:?} actor_id={}", e, actor_id);
                                }
                            }
                        }
                        ActorCommand::Query(query, responder) => {
                            let context_guard = actor_state.context.read().await;
                            // Use delegated handle_query
                            let result = actor_state.logic.handle_query(&actor_state.state, &context_guard, query).await;
                            let _ = responder.send(result);
                            log::debug!("Processed query actor_id={}", actor_id);
                        }
                         ActorCommand::GetSnapshot(responder) => {
                           // ... GetSnapshot logic remains the same ...
                        }
                        ActorCommand::Stop => {
                             log::info!("Stop command received actor_id={}", actor_id);
                            *actor_state.status.write().await = ActorStatus::Stopped;
                            break;
                        }
                    }
                }
                else => {
                     log::info!("Actor inbox closed, stopping. actor_id={}", actor_id);
                    break;
                }
            }
        }
        *actor_state.status.write().await = ActorStatus::Stopped;
        log::info!("Actor stopped actor_id={}", actor_id);
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

// --- create_actor ---
pub fn create_actor<L, C, S, E, I, Q, R>(
    logic: L,
    initial_state: S,
    ctx: C, // Use C directly
    actor_id: Option<Uuid>,
    buffer_size: usize,
) -> (
    // Use 5 type args for ActorRefImpl
    ActorRefImpl<E, Q, Result<R, StateError>, C, R>,
    JoinHandle<()>,
)
where
    L: ActorLogic<S, C, E, I, Q, Result<R, StateError>> + Send + Sync + 'static,
    S: StateTrait + Send + Sync + 'static + PartialEq,
    C: Send + Sync + 'static + Default + Clone + Debug, // Removed 'Context' trait bound
    E: EventTrait + Send + Sync + 'static,
    I: Send + Sync + Debug + 'static + Default,
    Q: Send + Sync + Debug + 'static,
    R: Send + Sync + Debug + 'static + Default,
{
    let id = actor_id.unwrap_or_else(Uuid::new_v4);
    let (sender, receiver): (
        // Use 5 type args for ActorCommand
        mpsc::Sender<ActorCommand<E, Q, Result<R, StateError>, C, R>>,
        mpsc::Receiver<ActorCommand<E, Q, Result<R, StateError>, C, R>>,
    ) = mpsc::channel(buffer_size);

    let actor_ref = ActorRefImpl {
        id,
        sender: sender.clone(),
        status: Arc::new(RwLock::new(ActorStatus::Active)),
        _phantom: PhantomData,
    };

    // Use 8 type args for ActorImpl
    let actor_instance: ActorImpl<L, C, E, S, I, Q, R, Result<R, StateError>> = ActorImpl {
        id,
        logic: Arc::new(logic), // Store Arc<L>
        initial_state,          // Store initial state
        context: ctx,           // Store initial context
        actor_id,
        buffer_size,
        inbox: receiver, // receiver is moved here
        status: actor_ref.status.clone(),
        snapshot: None,
        // PhantomData fields for C, I, R, Resp (Result<R, StateError>)
        _phantom_l: PhantomData,
        _phantom_e: PhantomData,
        _phantom_s: PhantomData,
        _phantom_i: PhantomData,
        _phantom_q: PhantomData,
        _phantom_r: PhantomData,
        _phantom_resp: PhantomData,
    };

    // Spawn task to run the actor's loop
    let handle = tokio::spawn(async move {
         // Call the run method which contains the loop
         actor_instance.run().await;
    });

    (actor_ref, handle)
}

// Implement ActorLogic for Arc<L> to allow calling methods on Arc<L>
#[async_trait]
impl<L, S, C, E, I, Q, Resp> ActorLogic<S, C, E, I, Q, Resp> for Arc<L>
where
    L: ActorLogic<S, C, E, I, Q, Resp> + Send + Sync + 'static,
    S: StateTrait + Send + Sync + 'static,
    C: Send + Sync + 'static + Default + Clone + Debug,
    E: EventTrait + Send + Sync + 'static,
    I: Send + Sync + 'static,
    Q: Send + Sync + 'static,
    Resp: Send + Sync + 'static,
{
    fn initial(&self) -> (S, C) {
        (**self).initial()
    }

    async fn transition(&self, state: S, context: C, event: E) -> Result<(S, C), StateError> {
        (**self).transition(state, context, event).await
    }

    async fn handle_query(&self, state: &S, context: &C, query: Q) -> Resp {
        (**self).handle_query(state, context, query).await
    }
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
    impl ActorLogic<TestState, Context, TestEvent, (), TestQuery, Result<TestResponse, StateError>>
        for TestActorLogic
    {
        fn initial(&self) -> (TestState, Context) {
            (
                TestState { count: 0, name: "initial".to_string() },
                Context::new()
            )
        }

        async fn transition(&self, mut state: TestState, _context: Context, event: TestEvent) -> Result<(TestState, Context), StateError>
        {
            match event {
                TestEvent::Increment => state.count += 1,
                TestEvent::Decrement => state.count -= 1,
                TestEvent::SetName(name) => state.name = name.clone(),
            }
            Ok((state, _context))
        }

        async fn handle_query(&self, state: &TestState, _context: &Context, query: TestQuery) -> Result<TestResponse, StateError>
        {
            match query {
                TestQuery::GetCount => Ok(TestResponse(format!("Count: {}", state.count))),
                TestQuery::GetName => Ok(TestResponse(format!("Name: {}", state.name))),
            }
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
            Context::new(),
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
            Context::new(),
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
            Context::new(),
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
