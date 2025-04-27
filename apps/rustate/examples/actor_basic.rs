use rustate::*;
use serde::{Deserialize, Serialize};
use std::fmt;

// 1. Define Context (Optional - using default empty context here)
type ToggleContext = Context;

// 2. Define Event Type
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
enum ToggleEvent {
    Toggle,
    #[default]
    None, // Default variant
}

// Implement EventTrait for our enum
impl EventTrait for ToggleEvent {
    fn name(&self) -> &str {
        match self {
            ToggleEvent::Toggle => "TOGGLE",
            ToggleEvent::None => "NONE",
        }
    }
    fn event_type(&self) -> &str {
        self.name()
    }
    fn payload(&self) -> Option<&serde_json::Value> {
        None
    }
}

// Implement IntoEvent
impl IntoEvent for ToggleEvent {
    fn into_event(self) -> Event {
        Event::new(self.name())
    }
}

impl fmt::Display for ToggleEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    println!("--- Basic Actor Example ---");

    // 3. Define the State Machine Logic
    let off_id = "off".to_string();
    let on_id = "on".to_string();

    let _toggle_machine: Machine<ToggleContext, ToggleEvent, String, ()> = MachineBuilder::new(
        "toggleMachine",
        off_id.clone(), // Initial state
    )
    .state(State::new(off_id.clone()))
    .state(State::new(on_id.clone()))
    .transition(Transition::new(
        off_id.clone(),
        Some(on_id.clone()),
        Some(ToggleEvent::Toggle),
        None,
        vec![],
        transition::TransitionType::External,
    ))
    .transition(Transition::new(
        on_id.clone(),
        Some(off_id.clone()),
        Some(ToggleEvent::Toggle),
        None,
        vec![],
        transition::TransitionType::External,
    ))
    .build()
    .await?; // Build is async

    // 4. Spawn the machine as an actor
    println!("Spawning actor...");

    // *** TODO: Replace with the actual new actor spawning mechanism ***
    // This part is likely incorrect as create_actor seems problematic/changed.
    // Let's assume a hypothetical Machine::spawn() for now, which might
    // return an ActorRefImpl or similar handle.

    // let actor_options = ActorOptions {
    //     input: None, // Assuming Input type I is ()
    //     id: Some("toggle-actor-1".to_string()),
    // };

    // Placeholder: We need the correct way to get an actor reference.
    // let actor_ref: ActorRefImpl<ToggleEvent, _, _, _, _> = /* machine.spawn(actor_options).await? */;
    // For now, we can't proceed with actor interaction without the correct spawning.

    println!("\nExample finished (actor part skipped due to API changes).");

    /*
    // 5. Get the initial snapshot
    let initial_snapshot_result = actor_ref.get_snapshot().await; // Assuming get_snapshot is async
    if let Ok(initial_snapshot) = initial_snapshot_result {
        println!(
            "Initial Snapshot: states = {:?}, context = {:?}",
            initial_snapshot.current_states, // Access snapshot fields
            initial_snapshot.context()
        );
        // Assertions need to work with HashSet<String>
        assert!(initial_snapshot.current_states.contains(&off_id));
        // assert_eq!(*initial_snapshot.status(), ActorStatus::Active); // Status check might differ
    } else {
        println!("Failed to get initial snapshot");
    }

    // 6. Send an event
    println!("\nSending TOGGLE event...");
    if let Err(e) = actor_ref.send_event(ToggleEvent::Toggle).await {
        println!("Failed to send event: {}", e);
    }

    tokio::time::sleep(Duration::from_millis(50)).await;

    // 7. Get the updated snapshot
    let next_snapshot_result = actor_ref.get_snapshot().await;
     if let Ok(next_snapshot) = next_snapshot_result {
        println!(
            "Next Snapshot: states = {:?}, context = {:?}",
            next_snapshot.current_states,
            next_snapshot.context()
        );
        assert!(next_snapshot.current_states.contains(&on_id));
    } else {
         println!("Failed to get next snapshot");
    }

    // 8. Send another event
    println!("\nSending TOGGLE event again...");
    if let Err(e) = actor_ref.send_event(ToggleEvent::Toggle).await {
        println!("Failed to send event: {}", e);
    }
    tokio::time::sleep(Duration::from_millis(50)).await;
    let final_snapshot_result = actor_ref.get_snapshot().await;
     if let Ok(final_snapshot) = final_snapshot_result {
        println!(
            "Final Snapshot: states = {:?}, context = {:?}",
            final_snapshot.current_states,
            final_snapshot.context()
        );
        assert!(final_snapshot.current_states.contains(&off_id));
    } else {
        println!("Failed to get final snapshot");
    }

    println!("\nExample finished.");
    */

    Ok(())
}
