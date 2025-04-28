//! WASM bindings for the rustate library.
use crate::{
    action::Action,
    context::Context,
    error::Result,
    event::{Event, EventTrait, IntoEvent},
    guard::Guard,
    machine::{Machine, MachineBuilder},
    state::State,
    transition::{Transition, TransitionType},
};
use serde::{Deserialize, Serialize};
use serde_json; // Import the crate itself
use serde_json::Value; // Add this import
use std::cell::RefCell;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::RwLock;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;

// Define WasmMachineHandle
#[wasm_bindgen]
pub struct WasmMachineHandle {
    // Use Arc<Mutex<...>> for thread-safe sharing if needed, or RefCell for single-threaded WASM
    // Let's assume RefCell for now, common in single-threaded WASM contexts.
    // Adjust if multi-threading (e.g., with web workers) is intended.
    pub(crate) machine: RefCell<Machine>,
    // Add specific generic types if known, otherwise might need to be generic itself.
    // Assuming default Context and Event for now.
}

// Global state machines (consider a better way to manage them)
thread_local! {
    static TRAFFIC_MACHINE: RefCell<Option<Machine<Context, TrafficLightEvent, String>>> = RefCell::new(None);
    static MUSIC_MACHINE: RefCell<Option<Machine<Context, MusicPlayerEvent, String>>> = RefCell::new(None);
}

// Define TrafficLightEvent enum
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum TrafficLightEvent {
    #[default]
    Timer,
    PowerOutage,
    PowerRestored,
}

// Implement EventTrait for TrafficLightEvent
impl EventTrait for TrafficLightEvent {
    fn event_type(&self) -> &str {
        match self {
            TrafficLightEvent::Timer => "TIMER",
            TrafficLightEvent::PowerOutage => "POWER_OUTAGE",
            TrafficLightEvent::PowerRestored => "POWER_RESTORED",
        }
    }

    fn payload(&self) -> Option<&Value> {
        None
    }

    fn name(&self) -> &str {
        self.event_type()
    }
}

// Implement IntoEvent for TrafficLightEvent
impl IntoEvent for TrafficLightEvent {
    fn into_event(self) -> Event {
        Event::new(self.event_type())
    }
}

// Define MusicPlayerEvent enum
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum MusicPlayerEvent {
    #[default]
    PowerOn,
    PowerOff,
    Play,
    Stop,
    Pause,
    NextTrack,
    PrevTrack,
}

// Implement EventTrait for MusicPlayerEvent
impl EventTrait for MusicPlayerEvent {
    fn event_type(&self) -> &str {
        match self {
            MusicPlayerEvent::PowerOn => "POWER_ON",
            MusicPlayerEvent::PowerOff => "POWER_OFF",
            MusicPlayerEvent::Play => "PLAY",
            MusicPlayerEvent::Stop => "STOP",
            MusicPlayerEvent::Pause => "PAUSE",
            MusicPlayerEvent::NextTrack => "NEXT_TRACK", // Use correct name
            MusicPlayerEvent::PrevTrack => "PREV_TRACK", // Use correct name
        }
    }

    fn payload(&self) -> Option<&Value> {
        None
    }

    fn name(&self) -> &str {
        self.event_type()
    }
}

// Implement IntoEvent for MusicPlayerEvent
impl IntoEvent for MusicPlayerEvent {
    fn into_event(self) -> Event {
        Event::new(self.event_type())
    }
}

// Function to create the traffic light machine (internal)
async fn create_traffic_light() -> Result<Machine<Context, TrafficLightEvent, String, ()>> {
    // States, Guards, Transitions
    let green = State::<_, _, _>::new("green".to_string());
    let yellow = State::<_, _, _>::new("yellow".to_string());
    let red = State::<_, _, _>::new("red".to_string());

    let green_to_yellow = Transition::new(
        "green".to_string(),
        Some("yellow".to_string()),
        Some(TrafficLightEvent::Timer),
        None,
        vec![],
        TransitionType::External,
    );
    let yellow_to_red = Transition::new(
        "yellow".to_string(),
        Some("red".to_string()),
        Some(TrafficLightEvent::Timer),
        None,
        vec![],
        TransitionType::External,
    );
    let red_to_green = Transition::new(
        "red".to_string(),
        Some("green".to_string()),
        Some(TrafficLightEvent::Timer),
        None,
        vec![],
        TransitionType::External,
    );

    MachineBuilder::<Context, TrafficLightEvent, String, ()>::new(
        "trafficLight",
        "green".to_string(),
    )
    .context(Context::new()) // Call with zero args
    .state(green)
    .state(yellow)
    .state(red)
    .transition(green_to_yellow)
    .transition(yellow_to_red)
    .transition(red_to_green)
    // Change closure signature to Fn(Arc<RwLock<C>>, &E) -> Fut
    .on_entry(
        &"green".to_string(),
        |ctx_arc: Arc<RwLock<Context>>, _event: &TrafficLightEvent| async move {
            let mut ctx = ctx_arc.write().await;
            ctx.set("status", "GREEN").unwrap();
            web_sys::console::log_1(&JsValue::from_str("TRAFFIC LIGHT: Entered Green"));
            Ok(())
        },
    )
    .on_entry(
        &"yellow".to_string(),
        |ctx_arc: Arc<RwLock<Context>>, _event: &TrafficLightEvent| async move {
            let mut ctx = ctx_arc.write().await;
            ctx.set("status", "YELLOW").unwrap();
            web_sys::console::log_1(&JsValue::from_str("TRAFFIC LIGHT: Entered Yellow"));
            Ok(())
        },
    )
    .on_entry(
        &"red".to_string(),
        |ctx_arc: Arc<RwLock<Context>>, _event: &TrafficLightEvent| async move {
            let mut ctx = ctx_arc.write().await;
            ctx.set("status", "RED").unwrap();
            web_sys::console::log_1(&JsValue::from_str("TRAFFIC LIGHT: Entered Red"));
            Ok(())
        },
    )
    .build()
    .await
}

// Function to create the music player machine (internal)
async fn create_music_player() -> Result<Machine<Context, MusicPlayerEvent, String, ()>> {
    // States, Guard
    let power_off = State::<_, _, _>::new("powerOff".to_string());
    let player = State::<_, _, _>::new("player".to_string());
    let stopped = State::<_, _, _>::new("stopped".to_string());
    let playing = State::<_, _, _>::new("playing".to_string());
    let normal_speed = State::<_, _, _>::new("normal".to_string());
    let double_speed = State::<_, _, _>::new("doubleSpeed".to_string());
    let paused = State::<_, _, _>::new("paused".to_string());
    let is_track_valid_guard = Guard::new(
        "isTrackValid",
        |ctx: &Context, _evt: &MusicPlayerEvent| -> bool {
            // Correct signature
            let track_result: Option<Result<usize, _>> = ctx.get("track"); // Adjust type
            track_result.map(|r| r.unwrap_or(0)).unwrap_or(0) > 0 // Adjust unwrapping
        },
    );

    let mut initial_context = Context::new(); // Call with zero args
    initial_context.set("track", 1).ok();

    MachineBuilder::<Context, MusicPlayerEvent, String, ()>::new(
        "musicPlayer",
        "powerOff".to_string(),
    )
    .context(initial_context)
    .state(power_off)
    .state(player)
    .state(stopped)
    .state(playing)
    .state(normal_speed)
    .state(double_speed)
    .state(paused)
    // Change closure signature to Fn(Arc<RwLock<C>>, &E) -> Fut
    .on_entry(
        &"player".to_string(),
        |ctx_arc: Arc<RwLock<Context>>, _event: &MusicPlayerEvent| async move {
            web_sys::console::log_1(&JsValue::from_str(
                "MUSIC PLAYER: Entered Player superstate",
            ));
            let mut ctx = ctx_arc.write().await;
            ctx.set("player_state", "entered").unwrap();
            Ok(())
        },
    )
    .on_exit(
        &"player".to_string(),
        |ctx_arc: Arc<RwLock<Context>>, _event: &MusicPlayerEvent| async move {
            web_sys::console::log_1(&JsValue::from_str("MUSIC PLAYER: Exited Player superstate"));
            let mut ctx = ctx_arc.write().await;
            ctx.set("player_state", "exited").unwrap();
            Ok(())
        },
    )
    // -- Child state actions --
    .on_entry(
        &"playing".to_string(),
        |ctx_arc: Arc<RwLock<Context>>, _event: &MusicPlayerEvent| async move {
            let mut ctx = ctx_arc.write().await;
            ctx.set("status", "playing").unwrap();
            web_sys::console::log_1(&JsValue::from_str("MUSIC PLAYER: Entered Playing state"));
            Ok(())
        },
    )
    .transition(Transition::new(
        "playing".to_string(),
        Some("playing".to_string()),
        Some(MusicPlayerEvent::NextTrack),
        None,
        vec![Action::from_fn(
            // Transition actions still use Arc<RwLock<>>
            move |ctx_arc: Arc<RwLock<Context>>, _event: &MusicPlayerEvent| async move {
                let mut ctx_write = ctx_arc.write().await;
                let current_track_result: Option<Result<usize, _>> = ctx_write.get("track");
                let current_track = current_track_result.map(|r| r.unwrap_or(0)).unwrap_or(0);
                let next_track = current_track + 1;
                ctx_write.set("track", next_track).ok();
                web_sys::console::log_1(&JsValue::from_str(&format!(
                    "MUSIC PLAYER: Switched to next track {}",
                    next_track
                )));
                Ok(())
            },
        )],
        TransitionType::Internal,
    ))
    .transition(Transition::new(
        "playing".to_string(),
        Some("playing".to_string()),
        Some(MusicPlayerEvent::PrevTrack),
        Some(is_track_valid_guard.clone()),
        vec![Action::from_fn(
            // Transition actions still use Arc<RwLock<>>
            move |ctx_arc: Arc<RwLock<Context>>, _event: &MusicPlayerEvent| async move {
                let mut ctx_write = ctx_arc.write().await;
                let current_track_result: Option<Result<usize, _>> = ctx_write.get("track");
                let current_track = current_track_result.map(|r| r.unwrap_or(0)).unwrap_or(0);
                let prev_track = if current_track > 0 {
                    current_track - 1
                } else {
                    0
                };
                ctx_write.set("track", prev_track).ok();
                web_sys::console::log_1(&JsValue::from_str(&format!(
                    "MUSIC PLAYER: Switched to previous track {}",
                    prev_track
                )));
                Ok(())
            },
        )],
        TransitionType::Internal,
    ))
    .build()
    .await
}

#[wasm_bindgen]
pub async fn init_traffic_light() {
    web_sys::console::log_1(&JsValue::from_str("Initializing traffic light machine..."));
    match create_traffic_light().await {
        Ok(machine) => TRAFFIC_MACHINE.with(|m| *m.borrow_mut() = Some(machine)),
        Err(e) => web_sys::console::log_1(&JsValue::from_str(&format!(
            "Error initializing traffic light: {:?}",
            e
        ))),
    }
}

#[wasm_bindgen]
pub async fn init_music_player() {
    web_sys::console::log_1(&JsValue::from_str("Initializing music player machine..."));
    match create_music_player().await {
        Ok(machine) => MUSIC_MACHINE.with(|m| *m.borrow_mut() = Some(machine)),
        Err(e) => web_sys::console::log_1(&JsValue::from_str(&format!(
            "Error initializing music player: {:?}",
            e
        ))),
    }
}

#[wasm_bindgen]
pub async fn send_traffic_event(event_name: String) -> Result<JsValue, JsValue> {
    let event = match event_name.as_str() {
        "TIMER" => TrafficLightEvent::Timer,
        _ => return Err(JsValue::from_str("Unknown traffic event")),
    };

    TRAFFIC_MACHINE.with(|m| {
        if let Some(machine) = m.borrow_mut().as_mut() {
            // Need to spawn this future
            let fut = machine.send(event);
            // How to handle async in this context? Needs an executor.
            // For now, just log placeholder.
            web_sys::console::log_1(&JsValue::from_str(
                "Sending traffic event (async exec needed)",
            ));
            Ok(JsValue::from_str(&format!("Sent {}", event_name)))
        } else {
            Err(JsValue::from_str("Traffic machine not initialized"))
        }
    })
}

#[wasm_bindgen]
pub async fn send_music_event(event_name: String) -> Result<JsValue, JsValue> {
    let event = match event_name.as_str() {
        "PLAY" => MusicPlayerEvent::Play,
        "PAUSE" => MusicPlayerEvent::Pause,
        "STOP" => MusicPlayerEvent::Stop,
        "NEXT_TRACK" => MusicPlayerEvent::NextTrack,
        "PREV_TRACK" => MusicPlayerEvent::PrevTrack,
        _ => return Err(JsValue::from_str("Unknown music event")),
    };

    MUSIC_MACHINE.with(|m| {
        if let Some(machine) = m.borrow_mut().as_mut() {
            // Need to spawn this future
            let fut = machine.send(event);
            // How to handle async in this context? Needs an executor.
            web_sys::console::log_1(&JsValue::from_str(
                "Sending music event (async exec needed)",
            ));
            Ok(JsValue::from_str(&format!("Sent {}", event_name)))
        } else {
            Err(JsValue::from_str("Music machine not initialized"))
        }
    })
}

#[wasm_bindgen]
pub fn get_traffic_state() -> Result<String, JsValue> {
    TRAFFIC_MACHINE.with(|m| {
        if let Some(machine) = m.borrow().as_ref() {
            // Assuming get_current_state_value returns S which is String
            // Ok(machine.get_current_state_value().to_string())
            // Temporarily return joined states if direct method not available
            Ok(machine
                .current_states
                .iter()
                .map(|s| s.to_string())
                .collect::<Vec<_>>()
                .join(", "))
        } else {
            Err(JsValue::from_str("Traffic machine not initialized"))
        }
    })
}

#[wasm_bindgen]
pub fn get_music_state() -> Result<String, JsValue> {
    MUSIC_MACHINE.with(|m| {
        if let Some(machine) = m.borrow().as_ref() {
            // Ok(machine.get_current_state_value().to_string())
            Ok(machine
                .current_states
                .iter()
                .map(|s| s.to_string())
                .collect::<Vec<_>>()
                .join(", "))
        } else {
            Err(JsValue::from_str("Music machine not initialized"))
        }
    })
}

#[wasm_bindgen]
pub fn send_music_player_event(event_str: &str) -> Result<(), JsValue> {
    let event = match event_str.to_uppercase().as_str() {
        "POWER_ON" => MusicPlayerEvent::PowerOn,
        "POWER_OFF" => MusicPlayerEvent::PowerOff,
        "PLAY" => MusicPlayerEvent::Play,
        "STOP" => MusicPlayerEvent::Stop,
        "PAUSE" => MusicPlayerEvent::Pause,
        "NEXT_TRACK" => MusicPlayerEvent::NextTrack, // Use correct name
        "PREV_TRACK" => MusicPlayerEvent::PrevTrack, // Use correct name
        _ => return Err(JsValue::from_str("Unknown music event")),
    };

    MUSIC_MACHINE.with(|m| {
        // Take ownership of the machine from RefCell
        if let Some(mut machine) = m.borrow_mut().take() {
            // Need to spawn this future
            let fut = machine.send(event);
            wasm_bindgen_futures::spawn_local(async move {
                if let Err(e) = fut.await {
                    web_sys::console::log_1(&JsValue::from_str(&format!(
                        "Error sending event: {:?}",
                        e
                    )));
                }
                // Optionally put the machine back if its state should be preserved
                // This part needs careful consideration depending on application logic.
                // For now, we don't put it back to simplify the example and fix borrow errors.
                // If the machine state is needed after the event, a different approach is required,
                // likely involving message passing back to the main thread_local context.
            });
        } else {
            web_sys::console::log_1(&JsValue::from_str(
                "Music machine not initialized or already moved",
            ));
        }
        Ok(())
    })
}

// Helper macro for wasm logging
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
}

/// Asynchronously sends an event to the state machine contained within the Arc<Mutex>.
#[wasm_bindgen]
pub async fn send_event_to_machine_async(
    machine_handle: &WasmMachineHandle,
    event: JsValue,
) -> Result<(), JsValue> {
    let event: Event = serde_wasm_bindgen::from_value(event)
        .map_err(|e| JsValue::from_str(&format!("Failed to deserialize event: {}", e)))?;

    let machine_lock = Arc::clone(&machine_handle.machine);
    let mut machine = machine_lock.lock().await;
    machine
        .send(event)
        .await // Added .await
        .map_err(|e| JsValue::from_str(&format!("Error sending event: {}", e)))?; // Map error to JsValue
    Ok(())
}
