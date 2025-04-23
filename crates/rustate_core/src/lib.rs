pub mod actor;
pub mod actor_ref;
pub mod logic;
pub mod simple_counter;
pub mod spawn;
pub mod system;

// 公開するものを選択
pub use actor::{Actor, ActorError};
pub use actor_ref::ActorRef;
pub use logic::ActorLogic;
pub use spawn::spawn;
pub use system::ActorSystem;

#[cfg(test)]
mod tests {
    use super::*;
    use simple_counter::{CounterActor, CounterEvent};
    use tokio::time::{sleep, Duration};

    // Import the macro
    use rustate_macros::create_machine;
    // Import necessary traits for derived types in the macro test
    use serde::{Serialize, Deserialize};
    use async_trait::async_trait; // For manual Actor impl if needed

    // --- Test for original counter actor ---
    #[tokio::test]
    async fn test_counter_actor_with_system() {
        println!("Creating ActorSystem...");
        let system = ActorSystem::new("test-system");
        println!("ActorSystem created: {:?}", system);

        println!("Spawning CounterActor using system...");
        let counter_ref = system.spawn_default(CounterActor::default());
        println!("CounterActor spawned with ref: {:?}", counter_ref);

        sleep(Duration::from_millis(10)).await;

        println!("Sending Increment event...");
        let res1 = counter_ref.send(CounterEvent::Increment).await;
        assert!(res1.is_ok());
        println!("Increment event sent.");

        sleep(Duration::from_millis(10)).await;

        println!("Sending Increment event again...");
        let res2 = counter_ref.send(CounterEvent::Increment).await;
        assert!(res2.is_ok());
        println!("Increment event sent.");

        sleep(Duration::from_millis(10)).await;

        println!("Sending Print event...");
        let res3 = counter_ref.send(CounterEvent::Print).await;
        assert!(res3.is_ok());
        println!("Print event sent.");

        sleep(Duration::from_millis(50)).await;

        // TODO: Add assertions using an 'ask' pattern
        println!("Original counter test finished. Check logs for actor output.");
    }

    // --- Test for the create_machine macro ---

    // Define dummy types needed for the macro invocation
    #[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
    struct MySimpleContext { count: i32 }
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    enum MySimpleEvent { Increment, Decrement } // Added Decrement for potential future use
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    enum MySimpleState { Idle, Active }

    #[test]
    fn test_simple_create_machine_macro_generation() {
        // Invoke the macro with the defined types and initial state
        create_machine!(
            MySimpleMachine, // Generated logic struct name
            Context = MySimpleContext,
            Event = MySimpleEvent,
            State = MySimpleState,
            initial: MySimpleState::Idle { // Initial state variant and context
                count: 0
            },
            // transitions, states etc. to be added later
        );

        println!("Invoked create_machine macro.");

        // Instantiate the generated logic struct
        let logic = MySimpleMachine::default();
        println!("Instantiated generated logic: {:?}", logic);

        // Test the generated initial() method
        let (initial_state, initial_context) = logic.initial();
        assert_eq!(initial_state, MySimpleState::Idle, "Initial state should be Idle");
        assert_eq!(initial_context, MySimpleContext { count: 0 }, "Initial context should match");

        println!("Generated initial state and context verified successfully.");

        // We can't easily test the async transition method here without a runtime
        // and more setup, but we know it's just a dummy print for now.
    }

    // Optional: Test using the generated logic with the spawn system (requires more setup)
    /*
    #[tokio::test]
    async fn test_spawn_generated_machine() {
        // Re-declare the types or put them in a common place
        #[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
        struct TestContext { val: String }
        #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
        enum TestEvent { Ping }
        #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
        enum TestState { Start }

        create_machine!(
            MyTestMachineLogic,
            Context = TestContext,
            Event = TestEvent,
            State = TestState,
            initial: TestState::Start { val: "init".to_string() }
        );

        // Need an Actor wrapper around the generated ActorLogic
        // This design needs refinement: How does ActorLogic (with State enum) map to Actor (with State = Context)?
        // Option 1: Actor holds both logic and current State enum
        // Option 2: Actor's State becomes (StateEnum, Context) tuple
        // Option 3: ActorLogic is the Actor itself (might complicate Actor trait)

        // --- Placeholder for Actor wrapper ---
        #[derive(Clone)]
        struct MyTestActor { logic: MyTestMachineLogic } // Simplistic wrapper

        // Manually implement Actor for the wrapper for now
        #[async_trait]
        impl Actor for MyTestActor {
             type State = TestContext; // Let Actor state be the context for now
             type Event = TestEvent;
             type Output = ();

             fn initial_state(&self) -> Self::State {
                 self.logic.initial().1 // Return initial context
             }

             async fn receive(&self, state: Self::State, event: Self::Event) -> Result<Self::State, ActorError> {
                 // PROBLEM: We don't know the current *internal* state (TestState::Start) here.
                 // The Actor::receive signature might need adjustment, or the spawn loop
                 // needs to manage the internal State enum alongside the Context.
                 println!("WARN: MyTestActor::receive called, but cannot properly call logic.transition yet.");
                 // Dummy call attempt (will likely misuse state)
                 // let internal_initial_state = self.logic.initial().0;
                 // let (next_internal_state, next_context) = self.logic.transition(internal_initial_state, state, event).await?;
                 // Ok(next_context)
                 Err(ActorError::ProcessingFailed("Actor wrapper cannot call logic properly yet".to_string()))
             }
        }
        // --- End Placeholder ---


        let system = ActorSystem::new("gen-test");
        // let actor_ref = system.spawn_default(MyTestActor { logic: MyTestMachineLogic::default() });
        // println!("Spawned generated machine actor: {:?}", actor_ref);
        // actor_ref.send(TestEvent::Ping).await.unwrap();
        // sleep(Duration::from_millis(50)).await;
        println!("Placeholder test for spawning generated machine finished.");
    }
    */

} 