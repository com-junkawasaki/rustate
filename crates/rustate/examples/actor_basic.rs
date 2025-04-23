use rustate::{*};
use std::time::Duration;

// 1. Define Context (Optional - using default empty context here)
type ToggleContext = Context; // Using the default Context

// 2. Define Event Type (Using simple string for now)
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum ToggleEvent {
    Toggle,
    Unknown, // For demonstration
}

// Implement EventObject for our enum
impl EventObject for ToggleEvent {
    fn event_type(&self) -> &str {
        match self {
            ToggleEvent::Toggle => "TOGGLE",
            ToggleEvent::Unknown => "UNKNOWN",
        }
    }
    // Basic conversion, adapt as needed
    fn from_str(s: &str) -> Self {
        match s {
            "TOGGLE" => ToggleEvent::Toggle,
            _ => ToggleEvent::Unknown,
        }
    }
    fn to_string(&self) -> String {
        self.event_type().to_string()
    }
}


#[tokio::main]
async fn main() -> Result<()> { // Using rustate::Result
    println!("--- Basic Actor Example ---");

    // 3. Define the State Machine Logic
    let toggle_machine = MachineBuilder::<ToggleContext, ToggleEvent>::new("toggle")
        .state(State::new("off"))
        .state(State::new("on"))
        .initial("off")
        .transition(Transition::new("off", "TOGGLE", "on"))
        .transition(Transition::new("on", "TOGGLE", "off"))
        .build()?; // Build the machine definition

    // 4. Create an Actor instance
    println!("Creating actor...");
    let actor_options = ActorOptions { input: None, id: Some("toggle-actor-1".to_string()) };
    let actor_ref = create_actor(toggle_machine, actor_options);
    println!("Actor created with ID: {}", actor_ref.id());

    // Allow the actor task to initialize fully (optional, for safety)
    tokio::time::sleep(Duration::from_millis(10)).await;

    // 5. Get the initial snapshot
    let initial_snapshot = actor_ref.get_snapshot();
    println!("Initial Snapshot: value = {}, context = {:?}", initial_snapshot.value(), initial_snapshot.context());
    assert!(initial_snapshot.is_in("off"));
    assert_eq!(*initial_snapshot.status(), ActorStatus::Active);

    // 6. Send an event
    println!("\nSending TOGGLE event...");
    actor_ref.send(ToggleEvent::Toggle)?;

    // Give the actor time to process the event
    tokio::time::sleep(Duration::from_millis(50)).await;

    // 7. Get the updated snapshot
    let next_snapshot = actor_ref.get_snapshot();
    println!("Next Snapshot:    value = {}, context = {:?}", next_snapshot.value(), next_snapshot.context());
    assert!(next_snapshot.is_in("on"));
    assert_eq!(*next_snapshot.status(), ActorStatus::Active);

    // 8. Send another event
    println!("\nSending TOGGLE event again...");
    actor_ref.send(ToggleEvent::Toggle)?;
    tokio::time::sleep(Duration::from_millis(50)).await;
    let final_snapshot = actor_ref.get_snapshot();
    println!("Final Snapshot:   value = {}, context = {:?}", final_snapshot.value(), final_snapshot.context());
    assert!(final_snapshot.is_in("off"));
    assert_eq!(*final_snapshot.status(), ActorStatus::Active);

    println!("\nExample finished.");

    // The actor task continues running in the background.
    // TODO: Demonstrate how to stop the actor using actor_ref.stop() when implemented.

    Ok(())
} 