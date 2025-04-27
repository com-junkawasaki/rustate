use rustate::{
    transition::TransitionType, Action, Context, Event, EventTrait, IntoEvent, Machine,
    MachineBuilder, State, Transition,
};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{sleep, Duration};

// Define Event Enum
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
enum TrafficEvent {
    Timer,
    #[default]
    None,
}

impl EventTrait for TrafficEvent {
    fn name(&self) -> &str {
        match self {
            TrafficEvent::Timer => "TIMER",
            TrafficEvent::None => "NONE",
        }
    }
    fn event_type(&self) -> &str {
        self.name()
    }
    fn payload(&self) -> Option<&serde_json::Value> {
        None
    }
}

impl IntoEvent for TrafficEvent {
    fn into_event(self) -> Event {
        Event::new(self.name())
    }
}

impl fmt::Display for TrafficEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

#[tokio::main]
async fn main() -> rustate::Result<()> {
    // Create a traffic light state machine
    let machine = create_traffic_light().await?;

    println!("Traffic light state machine created");
    println!("Current state: {:?}", machine.current_states);

    // Run the traffic light simulation
    run_simulation(machine).await?;

    Ok(())
}

async fn create_traffic_light() -> rustate::Result<Machine<Context, TrafficEvent, String, ()>> {
    let green_id = "green".to_string();
    let yellow_id = "yellow".to_string();
    let red_id = "red".to_string();

    // Create the states
    let green = State::new(green_id.clone());
    let yellow = State::new(yellow_id.clone());
    let red = State::new(red_id.clone());

    // Define some actions
    let log_green = Action::from_fn(
        |_ctx_arc: Arc<RwLock<Context>>, _evt: &TrafficEvent| async move {
            println!("Entering GREEN state - Go!")
        },
    );

    let log_yellow = Action::from_fn(
        |_ctx_arc: Arc<RwLock<Context>>, _evt: &TrafficEvent| async move {
            println!("Entering YELLOW state - Slow down!")
        },
    );

    let log_red = Action::from_fn(
        |_ctx_arc: Arc<RwLock<Context>>, _evt: &TrafficEvent| async move {
            println!("Entering RED state - Stop!")
        },
    );

    // Create the transitions
    let green_to_yellow = Transition::new(
        green_id.clone(),
        Some(yellow_id.clone()),
        Some(TrafficEvent::Timer),
        None,
        vec![log_yellow.clone()], // Action on transition
        TransitionType::External,
    );
    let yellow_to_red = Transition::new(
        yellow_id.clone(),
        Some(red_id.clone()),
        Some(TrafficEvent::Timer),
        None,
        vec![log_red.clone()], // Action on transition
        TransitionType::External,
    );
    let red_to_green = Transition::new(
        red_id.clone(),
        Some(green_id.clone()),
        Some(TrafficEvent::Timer),
        None,
        vec![log_green.clone()], // Action on transition
        TransitionType::External,
    );

    // Build the machine
    let machine = MachineBuilder::new("trafficLight".to_string(), green_id)
        .state(green)
        .state(yellow)
        .state(red)
        .transition(green_to_yellow)
        .transition(yellow_to_red)
        .transition(red_to_green)
        // Context is default, no need to add explicitly unless modified
        .build()
        .await?;

    Ok(machine)
}

async fn run_simulation(
    mut machine: Machine<Context, TrafficEvent, String, ()>,
) -> rustate::Result<()> {
    println!("\nStarting traffic light simulation...");
    println!("Press Ctrl+C to exit\n");

    loop {
        // Wait some time in the current state
        let wait_time = match machine.current_states.iter().next().map(|s| s.as_str()) {
            Some("green") => 3,
            Some("yellow") => 1,
            Some("red") => 2,
            _ => 1, // Default wait time
        };

        sleep(Duration::from_secs(wait_time)).await;

        // Send a timer event to transition to the next state
        machine.send(TrafficEvent::Timer).await?;
    }
}
