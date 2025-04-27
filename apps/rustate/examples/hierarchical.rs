use rustate::{
    transition::TransitionType, Action, Context, Event, EventTrait, IntoEvent, Machine,
    MachineBuilder, State, Transition,
};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::sync::Arc;
use tokio; // Need tokio runtime for async main
use tokio::sync::RwLock; // Import RwLock // Import Arc for shared ownership

// Define a custom Event type
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
enum MusicEvent {
    Power,
    Play,
    Stop,
    Pause,
    Next,
    Prev,
    SpeedUp,
    SpeedNormal,
    // Add a default event if necessary, or handle None case
    None,
}

impl fmt::Display for MusicEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

// Implement EventTrait for your custom event enum
impl EventTrait for MusicEvent {
    fn name(&self) -> &str {
        match self {
            MusicEvent::Power => "POWER",
            MusicEvent::Play => "PLAY",
            MusicEvent::Stop => "STOP",
            MusicEvent::Pause => "PAUSE",
            MusicEvent::Next => "NEXT",
            MusicEvent::Prev => "PREV",
            MusicEvent::SpeedUp => "SPEED_UP",
            MusicEvent::SpeedNormal => "SPEED_NORMAL",
            MusicEvent::None => "NONE",
        }
    }
    fn event_type(&self) -> &str {
        self.name()
    }
    fn payload(&self) -> Option<&serde_json::Value> {
        None
    }
}

// Implement IntoEvent for MusicEvent
impl IntoEvent for MusicEvent {
    fn into_event(self) -> Event {
        // Convert the enum variant into a standard Event struct
        Event::new(self.name()) // Assuming no payload for simplicity here
    }
}

// Implement Default for the custom event enum
impl Default for MusicEvent {
    fn default() -> Self {
        MusicEvent::None // Provide a sensible default
    }
}

// Use String for StateTrait
type PlayerState = String;

#[derive(Debug, Clone, PartialEq)]
pub struct MusicContext {
    track: usize,
}

// Define actions
fn next_track_action() -> Action<Context, MusicEvent> {
    Action::from_fn(|ctx_arc: Arc<RwLock<Context>>, _evt| {
        Box::pin(async move {
            // Fix: Handle Option<Result<usize, Error>>
            let current_track_result = ctx_arc.read().await.get::<usize>("track");
            let current_track = current_track_result
                .and_then(|res| res.ok()) // Get usize? if Ok
                .unwrap_or(0); // Default to 0
            let next_track = current_track + 1;
            println!("ACTION: Playing next track ({})", next_track);
            let _ = ctx_arc.write().await.set("track", next_track);
            Ok(())
        })
    })
}

fn prev_track_action() -> Action<Context, MusicEvent> {
    Action::from_fn(|ctx_arc: Arc<RwLock<Context>>, _evt| {
        Box::pin(async move {
            // Fix: Handle Option<Result<usize, Error>>
            let current_track_result = ctx_arc.read().await.get::<usize>("track");
            let current_track = current_track_result
                .and_then(|res| res.ok()) // Get usize? if Ok
                .unwrap_or(0); // Default to 0

            let prev_track = if current_track > 0 {
                current_track - 1
            } else {
                0
            };
            println!("ACTION: Playing previous track ({})", prev_track);
            let _ = ctx_arc.write().await.set("track", prev_track);
            Ok(())
        })
    })
}

#[tokio::main] // Use tokio::main for async
async fn main() -> rustate::Result<()> {
    // Create a hierarchical state machine for a music player
    let mut machine = create_player().await?; // Await the async builder

    println!("Music player state machine created");
    println!("Current states: {:?}", machine.current_states);

    // Send some events
    println!("\nSending PLAY event");
    machine.send(MusicEvent::Play).await?; // Use custom event enum and await
    println!("Current states: {:?}", machine.current_states);

    println!("\nSending NEXT event");
    machine.send(MusicEvent::Next).await?;
    println!("Current states: {:?}", machine.current_states);

    println!("\nSending PAUSE event");
    machine.send(MusicEvent::Pause).await?;
    println!("Current states: {:?}", machine.current_states);

    println!("\nSending PLAY event");
    machine.send(MusicEvent::Play).await?;
    println!("Current states: {:?}", machine.current_states);

    println!("\nSending POWER event");
    machine.send(MusicEvent::Power).await?;
    println!("Current states: {:?}", machine.current_states);

    Ok(())
}

// Make create_player async
async fn create_player() -> rustate::Result<Machine<Context, MusicEvent, PlayerState, ()>> {
    // Use String for State IDs
    let power_off = State::new("powerOff".to_string());

    // Initial state must be provided to new_compound
    let mut player = State::new_compound("player".to_string(), "stopped".to_string());
    // Parent is set implicitly when adding to StateCollection or MachineBuilder (no direct parent field)

    let stopped = State::new("stopped".to_string());
    // stopped.parent = Some("player".to_string()); // Remove manual parent setting

    // Initial state must be provided to new_compound
    let mut playing = State::new_compound("playing".to_string(), "normal".to_string());
    // playing.parent = Some("player".to_string()); // Remove manual parent setting

    let normal = State::new("normal".to_string());
    // normal.parent = Some("playing".to_string()); // Remove manual parent setting

    let double_speed = State::new("doubleSpeed".to_string());
    // double_speed.parent = Some("playing".to_string()); // Remove manual parent setting

    let paused = State::new("paused".to_string());
    // paused.parent = Some("player".to_string()); // Remove manual parent setting

    // Create actions first
    let log_power_on = Action::from_fn(
        |_ctx_arc: Arc<RwLock<Context>>, _evt: &MusicEvent| async move {
            println!("Power ON - Player ready");
            Ok(())
        },
    );
    let log_power_off = Action::from_fn(
        |_ctx_arc: Arc<RwLock<Context>>, _evt: &MusicEvent| async move {
            println!("Power OFF");
            Ok(())
        },
    );
    let log_playing = Action::from_fn(
        |_ctx_arc: Arc<RwLock<Context>>, _evt: &MusicEvent| async move {
            println!("Playing track...");
            Ok(())
        },
    );
    let log_stopped = Action::from_fn(
        |_ctx_arc: Arc<RwLock<Context>>, _evt: &MusicEvent| async move {
            println!("Stopped");
            Ok(())
        },
    );
    let log_paused = Action::from_fn(
        |_ctx_arc: Arc<RwLock<Context>>, _evt: &MusicEvent| async move {
            println!("Paused");
            Ok(())
        },
    );
    let log_double_speed = Action::from_fn(
        |_ctx_arc: Arc<RwLock<Context>>, _evt: &MusicEvent| async move {
            println!("Playing at double speed");
            Ok(())
        },
    );
    let log_normal_speed = Action::from_fn(
        |_ctx_arc: Arc<RwLock<Context>>, _evt: &MusicEvent| async move {
            println!("Playing at normal speed");
            Ok(())
        },
    );
    let next_track_action = next_track_action();
    let prev_track_action = prev_track_action();

    // Create transitions with full arguments (source, target, event, guard, actions, type)
    let power_toggle = Transition::new(
        "powerOff".to_string(),
        Some("player".to_string()), // Target
        Some(MusicEvent::Power),    // Event (wrapped in Some)
        None,                       // guard
        vec![log_power_on.clone()], // actions (pass directly)
        TransitionType::External,
    );
    let power_off_transition = Transition::new(
        "player".to_string(),
        Some("powerOff".to_string()), // Target
        Some(MusicEvent::Power),      // Event (wrapped in Some)
        None,
        vec![log_power_off.clone()], // actions
        TransitionType::External,
    );

    let play = Transition::new(
        "stopped".to_string(),
        Some("playing".to_string()), // Target
        Some(MusicEvent::Play),      // Event
        None,
        vec![log_playing.clone()], // actions
        TransitionType::External,
    );
    let stop = Transition::new(
        "playing".to_string(),
        Some("stopped".to_string()), // Target
        Some(MusicEvent::Stop),      // Event
        None,
        vec![log_stopped.clone()], // actions
        TransitionType::External,
    );
    let pause = Transition::new(
        "playing".to_string(),
        Some("paused".to_string()), // Target
        Some(MusicEvent::Pause),    // Event
        None,
        vec![log_paused.clone()], // actions
        TransitionType::External,
    );
    let resume = Transition::new(
        "paused".to_string(),
        Some("playing".to_string()), // Target
        Some(MusicEvent::Play),      // Event
        None,
        vec![log_playing.clone()], // actions (reuse log_playing)
        TransitionType::External,
    );

    let speed_up = Transition::new(
        "normal".to_string(),
        Some("doubleSpeed".to_string()), // Target
        Some(MusicEvent::SpeedUp),       // Event
        None,
        vec![log_double_speed.clone()], // actions
        TransitionType::External,
    );
    let speed_normal = Transition::new(
        "doubleSpeed".to_string(),
        Some("normal".to_string()),    // Target
        Some(MusicEvent::SpeedNormal), // Event
        None,
        vec![log_normal_speed.clone()], // actions
        TransitionType::External,
    );

    // Internal transitions
    let next_track_trans = Transition::new(
        "playing".to_string(),  // Source state for internal transition
        None::<PlayerState>,    // No target for internal (with type annotation)
        Some(MusicEvent::Next), // Event
        None,
        vec![next_track_action.clone()], // Actions passed directly
        TransitionType::Internal,
    );
    let prev_track_trans = Transition::new(
        "playing".to_string(),
        None::<PlayerState>,    // No target (with type annotation)
        Some(MusicEvent::Prev), // Event
        None,
        vec![prev_track_action.clone()], // Actions passed directly
        TransitionType::Internal,
    );

    // Create context with initial track
    let mut context = Context::new();
    let _ = context.set("track", 0);

    // Create and configure the state machine
    // Removed .with_action calls as actions are now passed directly

    // Use add_child for hierarchy
    player.add_child(stopped.clone());
    player.add_child(paused.clone()); // Moved paused up
    playing.add_child(normal.clone());
    playing.add_child(double_speed.clone());
    player.add_child(playing); // Add the 'playing' compound state as a child of 'player'

    // MachineBuilder::new needs the initial state ID and O type parameter
    let machine = MachineBuilder::<Context, MusicEvent, PlayerState, ()>::new(
        // Added O = ()
        "musicPlayer",
        "powerOff".to_string(), // Provide initial state ID here
    )
    .state(power_off)
    // Add player state (hierarchy defined above with add_child)
    .state(player) // Add the configured player state
    // Add leaf states (ensure they are added if not implicitly part of a parent)
    // Note: States added via add_child might not need explicit .state() here if MachineBuilder handles it.
    // Let's assume MachineBuilder is smart enough or add them just in case.
    .state(stopped)
    .state(paused)
    .state(normal)
    .state(double_speed)
    // Add transitions
    .transition(power_toggle)
    .transition(power_off_transition)
    .transition(play)
    .transition(stop)
    .transition(pause)
    .transition(resume)
    .transition(speed_up)
    .transition(speed_normal)
    .transition(next_track_trans) // Use transition with action already included
    .transition(prev_track_trans) // Use transition with action already included
    // Remove on_entry calls as actions are now part of transitions
    .context(context)
    .build() // Build is now async
    .await?; // Await the build result

    Ok(machine)
}
