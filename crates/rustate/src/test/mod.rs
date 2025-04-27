pub mod checker;
pub mod generator;
pub mod property;
pub mod runner;
#[cfg(feature = "xstate-compat")]
pub mod xstate;

pub use checker::{ModelChecker, Property, PropertyType, VerificationResult};
pub use generator::{TestCase, TestGenerator};
pub use runner::{CoverageReport, TestResult, TestResults, TestRunner};

// プロパティベースドテストモジュールからneed-to-exportのみをexport
#[cfg(feature = "property-testing")]
pub use property::{
    EventSequenceStrategyBuilder, PropertyTestResult, PropertyTestRunner, StateMachineProperty,
};

// XState v5 互換モジュールをexport
#[cfg(feature = "xstate-compat")]
pub use xstate::{
    create_test_model, execute_test_plan, XStatePathSegment, XStateTestModel, XStateTestPath,
    XStateTestPlan,
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        error::StateError as Error, prelude::*, Context, Event, StateTrait, TransitionType,
    };
    use futures::executor::block_on;
    use serde::{Deserialize, Serialize};
    use serde_json::json;
    use std::collections::HashMap;
    use std::sync::Arc;
    use tokio::sync::RwLock;

    // Example State implementation (if needed, or use String)
    // #[derive(Serialize, Deserialize, Clone, Debug, Hash, Eq, PartialEq)]
    // enum TestState { Idle, Running, Completed }
    // impl StateTrait for TestState {}

    // Example Event implementation (if needed, or use Event)
    // #[derive(Clone, Debug, Hash, Eq, PartialEq)]
    // enum TestEvent { Start, Complete, Reset }
    // impl EventTrait for TestEvent {}

    // --- Test setup for a simple async machine --- (Starts around line 43)
    fn setup_simple_async_machine() -> Machine<Context, Event, String> {
        // Define states using String::from()
        let idle_state = State::new(String::from("idle"));
        let running_state = State::new(String::from("running"));
        // Mark completed state as final using .make_final()
        let completed_state = State::new(String::from("completed")).make_final();

        // Define actions using Action::from_fn
        let increment_action = Action::from_fn(|ctx: Arc<RwLock<Context>>, _evt: &Event| {
            async move {
                let mut context_guard = ctx.write().await;
                let current = context_guard
                    .get::<i32>("count")
                    .ok()
                    .flatten()
                    .unwrap_or(0);
                context_guard
                    .set("count", current + 1)
                    .map_err(|e| Error::Context(e.to_string()))
                // FIXME: Proper error handling for set
            }
        });

        // Define transitions using the full signature and String::from()
        let start_transition = Transition::new(
            String::from("idle"),          // source
            Some(String::from("running")), // target
            Some(Event::new("START")),
            None,                           // guard
            vec![increment_action.clone()], // actions
            TransitionType::External,       // type
        );
        let complete_transition = Transition::new(
            String::from("running"),
            Some(String::from("completed")),
            Some(Event::new("COMPLETE")),
            None,
            vec![increment_action.clone()],
            TransitionType::External,
        );
        let reset_transition = Transition::new(
            String::from("completed"),
            Some(String::from("idle")),
            Some(Event::new("RESET")),
            None,
            vec![], // No actions for reset
            TransitionType::External,
        );

        // Build the machine using MachineBuilder::new with initial state
        let machine_builder = MachineBuilder::new("testMachine", String::from("idle"))
            .state(idle_state)
            .state(running_state)
            .state(completed_state) // Add the final state
            .transition(start_transition)
            .transition(complete_transition)
            .transition(reset_transition);

        // Build synchronously for setup function
        block_on(machine_builder.build()).unwrap()
    }

    #[tokio::test]
    async fn test_simple_async_cycle() {
        let mut machine = setup_simple_async_machine();
        assert!(machine.is_in(&"idle".to_string()));
        let context_guard = machine.context.read().await.unwrap();
        assert_eq!(context_guard.get::<i32>("count").unwrap_or_default(), 0);
        drop(context_guard);

        let result = machine.send(Event::new("START")).await;
        assert!(result.is_ok());
        assert!(machine.is_in(&"running".to_string()));
        let context_guard = machine.context.read().await.unwrap();
        assert_eq!(context_guard.get::<i32>("count").unwrap_or_default(), 1);
        drop(context_guard);

        let result = machine.send(Event::new("COMPLETE")).await;
        assert!(result.is_ok());
        assert!(machine.is_in(&"completed".to_string()));
        assert!(machine.current_states.iter().any(|s| s == "completed"));
        let context_guard = machine.context.read().await.unwrap();
        assert_eq!(context_guard.get::<i32>("count").unwrap_or_default(), 1);
        drop(context_guard);

        let result = machine.send(Event::new("RESET")).await;
        assert!(result.is_ok());
        assert!(machine.is_in(&"idle".to_string()));
        assert!(!machine.current_states.iter().any(|s| s == "completed"));
        let context_guard = machine.context.read().await.unwrap();
        assert_eq!(context_guard.get::<i32>("count").unwrap_or_default(), 0);
        drop(context_guard);
    }
    // --- End of simple async machine test --- (Around line 120)

    // --- Test setup for traffic light machine --- (Starts around line 167)
    fn setup_traffic_light_machine() -> Machine<Context, Event, String> {
        // Define states using String::from()
        let green_state = State::new(String::from("green"));
        let yellow_state = State::new(String::from("yellow"));
        let red_state = State::new(String::from("red"));
        let maintenance_state = State::new(String::from("maintenance"));

        // Define guards
        let is_timer_expired =
            Guard::new("is_timer_expired", |ctx: &Context, _evt: &Event| -> bool {
                ctx.get::<i32>("timer")
                    .and_then(|r| r.ok())
                    .unwrap_or_default()
                    >= 5 // Use unwrap_or_default
                         // FIXME: Proper Result handling needed
            });
        let is_maintenance_mode = Guard::new(
            "is_maintenance_mode",
            |ctx: &Context, _evt: &Event| -> bool {
                ctx.get::<bool>("maintenance")
                    .and_then(|r| r.ok())
                    .unwrap_or_default() // Use unwrap_or_default
            },
        );

        // Define actions using Action::from_fn
        let increment_timer = Action::from_fn(|ctx, _evt| {
            async move {
                let mut context_guard = ctx.write().await;
                let current = context_guard
                    .get::<i32>("timer")
                    .and_then(|r| r.ok())
                    .unwrap_or_default(); // Use unwrap_or_default for tests
                context_guard
                    .set("timer", current + 1)
                    .map_err(|e| Error::ContextError(e.to_string())) // Use ContextError
            }
        });
        let reset_timer = Action::from_fn(|ctx, _evt| {
            async move {
                ctx.write()
                    .await
                    .set("timer", 0)
                    .map_err(|e| Error::ContextError(e.to_string())) // Use ContextError
            }
        });
        let set_maintenance = Action::from_fn(|ctx, _evt| {
            async move {
                ctx.write()
                    .await
                    .set("maintenance", true)
                    .map_err(|e| Error::ContextError(e.to_string())) // Use ContextError
                                                                     // FIXME: Proper Result handling needed
            }
        });
        let clear_maintenance =
            Action::from_fn(|ctx: Arc<RwLock<Context>>, _evt: &Event| async move {
                let mut context = ctx.write().await;
                context
                    .set("maintenance", false)
                    .map_err(|e| Error::ContextError(e.to_string()))?
            });

        // Define transitions with full arguments and String::from()
        let green_to_yellow = Transition::new(
            String::from("green"),         // source
            Some(String::from("yellow")),  // target
            Some(Event::from("TIMER")),    // Use Event::from, not Event::new
            None,                          // guard
            vec![increment_timer.clone()], // actions
            TransitionType::External,      // type
        );
        let yellow_to_red = Transition::new(
            String::from("yellow"),
            Some(String::from("red")),
            Some(Event::from("TIMER")), // Use Event::from, not Event::new
            None,
            vec![increment_timer.clone()],
            TransitionType::External,
        );
        let red_to_green = Transition::new(
            String::from("red"),
            Some(String::from("green")), // Back to green
            Some(Event::from("TIMER")),  // Use Event::from, not Event::new
            None,
            vec![increment_timer.clone()],
            Some(String::from("green")),
            Some(Event::from("TIMER")),
            Some(is_timer_expired.clone()), // Guard was missing
            vec![reset_timer.clone()],
            TransitionType::External,
        );
        // Wildcard source might need StateTrait implementation or specific handling
        let to_maintenance = Transition::new(
            String::from("*"), // source
            Some(String::from("maintenance")),
            Some(Event::from("MAINTENANCE")), // Use Event::from, not Event::new
            None,                             // guard
            vec![set_maintenance.clone()],    // actions
            TransitionType::External,
        );
        let from_maintenance = Transition::new(
            String::from("maintenance"),
            Some(String::from("green")),       // target state
            Some(Event::from("RESTORE")),      // Use Event::from, not Event::new
            Some(is_maintenance_mode.clone()), // Guard
            vec![clear_maintenance.clone()],   // Actions
            TransitionType::External,
        );

        // Build the machine with initial state
        let machine_builder = MachineBuilder::new("trafficLight", String::from("green"))
            .state(green_state)
            .state(yellow_state)
            .state(red_state)
            .state(maintenance_state)
            .transition(green_to_yellow)
            .transition(yellow_to_red)
            .transition(red_to_green)
            .transition(to_maintenance)
            .transition(from_maintenance)
            // Define entry actions if needed (Action::new had ActionType::Entry before)
            .on_entry(String::from("red"), increment_timer); // Assuming state ID is String

        // Build synchronously for setup function
        block_on(machine_builder.build()).unwrap()
    }

    #[tokio::test]
    async fn test_traffic_light_cycle() {
        let mut machine = setup_traffic_light_machine(); // Not async anymore
        assert!(machine.is_in(&"green".to_string()));

        for _ in 0..10 {
            machine.send(Event::from("TIMER")).await.unwrap();
        }
        assert!(machine.is_in(&"yellow".to_string()));

        for _ in 0..10 {
            machine.send(Event::from("TIMER")).await.unwrap();
        }
        assert!(machine.is_in(&"red".to_string()));

        for _ in 0..10 {
            machine.send(Event::from("TIMER")).await.unwrap();
        }
        assert!(machine.is_in(&"green".to_string()));
    }

    #[tokio::test]
    async fn test_maintenance_mode() {
        let mut machine = setup_traffic_light_machine(); // Not async anymore
        assert!(machine.is_in(&"green".to_string()));

        let current_state_ids: Vec<_> = machine.current_states.iter().cloned().collect();
        println!("Current states before MAINTENANCE: {:?}", current_state_ids);

        let result = machine.send(Event::from("MAINTENANCE")).await;
        println!("MAINTENANCE event result: {:?}", result);
        assert!(result.is_ok());

        let current_state_ids_after: Vec<_> = machine.current_states.iter().cloned().collect();
        println!(
            "Current states after MAINTENANCE: {:?}",
            current_state_ids_after
        );
        assert!(machine.is_in(&"maintenance".to_string()));

        machine.send(Event::from("RESTORE")).await.unwrap();
        assert!(machine.is_in(&"green".to_string()));

        for _ in 0..10 {
            machine.send(Event::from("TIMER")).await.unwrap();
        }
        assert!(machine.is_in(&"yellow".to_string()));

        let result = machine.send(Event::from("MAINTENANCE")).await;
        assert!(result.is_ok());
        assert!(machine.is_in(&"maintenance".to_string()));

        machine.send(Event::from("RESTORE")).await.unwrap();
        assert!(machine.is_in(&"green".to_string()));
    }
    // --- End of traffic light machine test --- (Around line 250)

    // ... (Other tests like property tests might follow)
}

#[cfg(test)]
mod advanced_tests {
    use super::*;
    use crate::{
        test::{checker::*, generator::*, runner::*},
        Action, ActionType, Context, Event, Guard, Machine, MachineBuilder, State, Transition,
    };
    use tokio;

    #[cfg(feature = "property-testing")]
    use crate::test::property::*;

    /*
    #[test]
    fn test_model_checking_traffic_light() {
        let machine = create_traffic_light_machine();
        let mut checker = ModelChecker::new(&machine);

        let all_states_reachable = Property {
            name: "All states are reachable".to_string(),
            property_type: PropertyType::Reachability,
            target_states: vec![
                "green".to_string(),
                "yellow".to_string(),
                "red".to_string(),
                "maintenance".to_string()
            ],
            description: Some("All defined states should be reachable".to_string()),
        };

        let result = checker.verify_property(&all_states_reachable);
        assert!(result.satisfied, "Not all states are reachable: {:#?}", result);

        let invalid_states = Property {
            name: "No invalid states".to_string(),
            property_type: PropertyType::Safety,
            target_states: vec!["invalid".to_string(), "error".to_string()],
            description: Some("System should never reach undefined states".to_string()),
        };

        let result = checker.verify_property(&invalid_states);
        assert!(result.satisfied, "System can reach invalid states: {:#?}", result);
    }
    */

    /*
    #[test]
    fn test_generate_test_cases_for_traffic_light() {
        let machine = create_traffic_light_machine();
        let mut generator = TestGenerator::new(&machine);

        let state_tests = generator.generate_all_states();
        assert_eq!(state_tests.len(), 4, "Should generate test for all 4 states");

        let transition_tests = generator.generate_all_transitions();
        assert_eq!(transition_tests.len(), 5, "Should generate test for all 5 transitions");

        let mut runner = TestRunner::new(&machine);
        let results = runner.run_tests(transition_tests);

        assert!(results.all_passed(), "Not all transition tests passed: {:?}", results);
    }
    */
}
