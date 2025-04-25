//! WASM bindings for the rustate library.
#![cfg(feature = "wasm")]
use crate::{
    action::{Action, ActionType},
    context::Context,
    error::Result as StateResult, // Alias Result to avoid conflict with JS Result
    event::EventTrait,
    guard::Guard,
    machine::MachineBuilder,
    state::{State, StateType},
    transition::{Transition, TransitionType},
    Event, // Use crate::Event
    IntoEvent,
    Machine, // Use crate::Machine
};
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use wasm_bindgen::prelude::*;
use web_sys::console;

// Global state machines (consider a better way to manage them)
thread_local! {
    static TRAFFIC_MACHINE: RefCell<Option<Machine<Context, TrafficLightEvent, String>>> = RefCell::new(None);
    static MUSIC_MACHINE: RefCell<Option<Machine<Context, MusicPlayerEvent, String>>> = RefCell::new(None);
}

// Define TrafficLightEvent enum
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TrafficLightEvent {
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
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MusicPlayerEvent {
    Play,
    Pause,
    Stop,
    Next,
    Previous,
}

// Implement EventTrait for MusicPlayerEvent
impl EventTrait for MusicPlayerEvent {
    fn event_type(&self) -> &str {
        match self {
            MusicPlayerEvent::Play => "PLAY",
            MusicPlayerEvent::Pause => "PAUSE",
            MusicPlayerEvent::Stop => "STOP",
            MusicPlayerEvent::Next => "NEXT",
            MusicPlayerEvent::Previous => "PREVIOUS",
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
fn create_traffic_light() -> StateResult<Machine<Context, TrafficLightEvent, String>> {
    // ... (States, Transitions, Actions defined using TrafficLightEvent) ...
    let green = State::<String, Context, TrafficLightEvent>::new("green".to_string());
    let yellow = State::<String, Context, TrafficLightEvent>::new("yellow".to_string());
    let red = State::<String, Context, TrafficLightEvent>::new("red".to_string());

    // Transitions...
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

    // Actions...
    let log_green = Action::new(
        "logGreen",
        ActionType::Entry,
        |_ctx: &mut Context, _evt: &TrafficLightEvent| {
            Box::pin(async move {
                console::log_1(&"Entering GREEN state - Go!".into());
            })
        },
    );
    let log_yellow = Action::new(
        "logYellow",
        ActionType::Entry,
        |_ctx: &mut Context, _evt: &TrafficLightEvent| {
            Box::pin(async move {
                console::log_1(&"Entering YELLOW state - Slow down!".into());
            })
        },
    );
    let log_red = Action::new(
        "logRed",
        ActionType::Entry,
        |_ctx: &mut Context, _evt: &TrafficLightEvent| {
            Box::pin(async move {
                console::log_1(&"Entering RED state - Stop!".into());
            })
        },
    );

    // Build
    MachineBuilder::<Context, TrafficLightEvent, String>::new(
        "trafficLight".to_string(),
        "green".to_string(),
    )
    .state(green)
    .state(yellow)
    .state(red)
    .transition(green_to_yellow)
    .transition(yellow_to_red)
    .transition(red_to_green)
    .action(log_green)
    .action(log_yellow)
    .action(log_red)
    // Assuming add_state_action or similar exists to link actions to state entry/exit
    // .add_state_action("green", ActionType::Entry, "logGreen")
    // .add_state_action("yellow", ActionType::Entry, "logYellow")
    // .add_state_action("red", ActionType::Entry, "logRed")
    .build()
}

// Function to create the music player machine (internal)
fn create_music_player() -> StateResult<Machine<Context, MusicPlayerEvent, String>> {
    // ... (States, Transitions, Actions defined using MusicPlayerEvent) ...
    let mut power_off = State::<String, Context, MusicPlayerEvent>::new("powerOff".to_string());
    let mut player = State::<String, Context, MusicPlayerEvent>::new("player".to_string());
    let mut stopped = State::<String, Context, MusicPlayerEvent>::new("stopped".to_string());
    stopped.parent = Some("player".to_string());
    let mut playing = State::<String, Context, MusicPlayerEvent>::new("playing".to_string());
    playing.parent = Some("player".to_string());
    let mut normal_speed = State::<String, Context, MusicPlayerEvent>::new("normal".to_string());
    normal_speed.parent = Some("playing".to_string());
    let mut double_speed =
        State::<String, Context, MusicPlayerEvent>::new("doubleSpeed".to_string());
    double_speed.parent = Some("playing".to_string());
    let mut paused = State::<String, Context, MusicPlayerEvent>::new("paused".to_string());
    paused.parent = Some("player".to_string());

    // Transitions...
    let power_toggle = Transition::new(
        "powerOff".to_string(),
        Some("player".to_string()),
        Some(MusicPlayerEvent::Play),
        None,
        vec![],
        TransitionType::External,
    );
    let power_off_transition = Transition::new(
        "player".to_string(),
        Some("powerOff".to_string()),
        Some(MusicPlayerEvent::Pause),
        None,
        vec![],
        TransitionType::External,
    );
    let play = Transition::new(
        "stopped".to_string(),
        Some("playing".to_string()),
        Some(MusicPlayerEvent::Play),
        None,
        vec![],
        TransitionType::External,
    );
    let stop = Transition::new(
        "playing".to_string(),
        Some("stopped".to_string()),
        Some(MusicPlayerEvent::Stop),
        None,
        vec![],
        TransitionType::External,
    );
    let pause = Transition::new(
        "playing".to_string(),
        Some("paused".to_string()),
        Some(MusicPlayerEvent::Pause),
        None,
        vec![],
        TransitionType::External,
    );
    let resume = Transition::new(
        "paused".to_string(),
        Some("playing".to_string()),
        Some(MusicPlayerEvent::Play),
        None,
        vec![],
        TransitionType::External,
    );
    let speed_up = Transition::new(
        "normal".to_string(),
        Some("doubleSpeed".to_string()),
        Some(MusicPlayerEvent::Play),
        None,
        vec![],
        TransitionType::External,
    );
    let speed_normal = Transition::new(
        "doubleSpeed".to_string(),
        Some("normal".to_string()),
        Some(MusicPlayerEvent::Play),
        None,
        vec![],
        TransitionType::External,
    );

    // Actions...
    let power_on_action = Action::new(
        "powerOn",
        ActionType::Entry,
        |_ctx: &mut Context, _evt: &MusicPlayerEvent| {
            Box::pin(async move { console::log_1(&"Power ON - Player ready".into()) })
        },
    );
    let power_off_action = Action::new(
        "powerOff",
        ActionType::Exit,
        |_ctx: &mut Context, _evt: &MusicPlayerEvent| {
            Box::pin(async move { console::log_1(&"Power OFF".into()) })
        },
    );
    let play_action = Action::new(
        "playMusic",
        ActionType::Entry,
        |_ctx: &mut Context, _evt: &MusicPlayerEvent| {
            Box::pin(async move { console::log_1(&"Playing track".into()) })
        },
    );
    let next_track_action = Action::new(
        "nextTrack",
        ActionType::Transition,
        |ctx: &mut Context, _evt: &MusicPlayerEvent| {
            Box::pin(async move {
                let current_track = ctx.get::<usize>("track").unwrap_or(0);
                let next_track = current_track + 1;
                console::log_1(&format!("Changing to track {}", next_track).into());
                let _ = ctx.set("track", next_track);
            })
        },
    );
    let prev_track_action = Action::new(
        "prevTrack",
        ActionType::Transition,
        |ctx: &mut Context, _evt: &MusicPlayerEvent| {
            Box::pin(async move {
                let current_track = ctx.get::<usize>("track").unwrap_or(0);
                let prev_track = if current_track > 0 {
                    current_track - 1
                } else {
                    0
                };
                console::log_1(&format!("Changing to track {}", prev_track).into());
                let _ = ctx.set("track", prev_track);
            })
        },
    );

    // Guard...
    let is_track_valid_guard =
        Guard::new("isTrackValid", |ctx: &Context, _evt: &MusicPlayerEvent| {
            ctx.get::<usize>("track")
                .map_or(false, |track_num| track_num > 0)
        });

    // Build
    MachineBuilder::<Context, MusicPlayerEvent, String>::new(
        "musicPlayer".to_string(),
        "powerOff".to_string(),
    )
    .state(power_off)
    .state(player)
    .state(stopped)
    .state(playing)
    .state(normal_speed)
    .state(double_speed)
    .state(paused)
    .transition(power_toggle)
    .transition(power_off_transition)
    .transition(play)
    .transition(stop)
    .transition(pause)
    .transition(resume)
    .transition(speed_up)
    .transition(speed_normal)
    // Transition with guard and action
    .transition(Transition::new(
        "stopped".to_string(),
        Some("playing".to_string()),
        Some(MusicPlayerEvent::Next),
        Some(is_track_valid_guard.clone()),
        vec![next_track_action.clone()],
        TransitionType::External,
    ))
    .transition(Transition::new(
        "stopped".to_string(),
        Some("playing".to_string()),
        Some(MusicPlayerEvent::Previous),
        None,
        vec![prev_track_action.clone()],
        TransitionType::External,
    ))
    .action(power_on_action)
    .action(power_off_action)
    .action(play_action)
    .action(next_track_action)
    .action(prev_track_action)
    .guard(is_track_valid_guard) // Add guard using .guard()
    // Assuming add_state_action or similar exists...
    // .add_state_action("player", ActionType::Entry, "powerOn")
    // .add_state_action("powerOff", ActionType::Entry, "powerOff") // Likely Exit action for player instead?
    // .add_state_action("playing", ActionType::Entry, "playMusic")
    .build()
}

#[wasm_bindgen]
pub fn init_traffic_light() {
    match create_traffic_light() {
        Ok(machine) => TRAFFIC_MACHINE.with(|m| *m.borrow_mut() = Some(machine)),
        Err(e) => console::error_1(&format!("Failed to init traffic light: {:?}", e).into()),
    }
}

#[wasm_bindgen]
pub fn init_music_player() {
    match create_music_player() {
        Ok(machine) => MUSIC_MACHINE.with(|m| *m.borrow_mut() = Some(machine)),
        Err(e) => console::error_1(&format!("Failed to init music player: {:?}", e).into()),
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
            console::log_1(&"Sending traffic event (async exec needed)".into());
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
        "NEXT" => MusicPlayerEvent::Next,
        "PREVIOUS" => MusicPlayerEvent::Previous,
        _ => return Err(JsValue::from_str("Unknown music event")),
    };

    MUSIC_MACHINE.with(|m| {
        if let Some(machine) = m.borrow_mut().as_mut() {
            // Need to spawn this future
            let fut = machine.send(event);
            // How to handle async in this context? Needs an executor.
            console::log_1(&"Sending music event (async exec needed)".into());
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
