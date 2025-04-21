#![cfg(feature = "wasm")]

use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::console;

use crate::{Action, ActionType, Context, Machine, MachineBuilder, State, Transition};
use std::cell::RefCell;

// グローバルな状態を保持するためのラッパー
thread_local! {
    static TRAFFIC_MACHINE: RefCell<Option<Machine>> = RefCell::new(None);
    static MUSIC_MACHINE: RefCell<Option<Machine>> = RefCell::new(None);
}

#[wasm_bindgen(start)]
pub fn start() {
    #[cfg(debug_assertions)]
    console_error_panic_hook::set_once();

    console::log_1(&"RuState Wasm initialized".into());
}

#[wasm_bindgen]
pub fn init_traffic_light() -> Result<(), JsValue> {
    let machine = create_traffic_light().map_err(|e| JsValue::from_str(&e.to_string()))?;

    TRAFFIC_MACHINE.with(|cell| {
        *cell.borrow_mut() = Some(machine);
    });

    console::log_1(&"Traffic light machine initialized".into());
    update_traffic_light_state();

    Ok(())
}

#[wasm_bindgen]
pub fn send_traffic_light_event(event: &str) -> Result<(), JsValue> {
    let mut success = false;

    TRAFFIC_MACHINE.with(|cell| {
        if let Some(machine) = &mut *cell.borrow_mut() {
            // エラーを処理するが関数からは伝播させない
            if let Err(e) = machine.send(event) {
                console::log_1(&format!("Error sending event: {}", e).into());
            } else {
                success = true;
            }
        }
    });

    if success {
        update_traffic_light_state();
        Ok(())
    } else {
        Err(JsValue::from_str("Failed to send event"))
    }
}

fn update_traffic_light_state() {
    TRAFFIC_MACHINE.with(|cell| {
        if let Some(machine) = &*cell.borrow() {
            let state = machine
                .current_states
                .iter()
                .next()
                .map_or("unknown", |s| s.as_str());
            console::log_2(&"Current state:".into(), &state.into());

            // JS側から呼び出される関数を設定（実際のDOM操作はJS側で行う）
            if let Some(update_fn) =
                js_sys::Reflect::get(&js_sys::global(), &"updateTrafficLightUI".into()).ok()
            {
                if update_fn.is_function() {
                    let function = update_fn.dyn_into::<js_sys::Function>().unwrap();
                    let _ = js_sys::Reflect::apply(
                        &function,
                        &JsValue::NULL,
                        &js_sys::Array::of1(&JsValue::from_str(state)),
                    );
                }
            }
        }
    });
}

#[wasm_bindgen]
pub fn init_music_player() -> Result<(), JsValue> {
    let machine = create_music_player().map_err(|e| JsValue::from_str(&e.to_string()))?;

    MUSIC_MACHINE.with(|cell| {
        *cell.borrow_mut() = Some(machine);
    });

    console::log_1(&"Music player machine initialized".into());
    update_music_player_state();

    Ok(())
}

#[wasm_bindgen]
pub fn send_music_player_event(event: &str) -> Result<(), JsValue> {
    let mut success = false;

    MUSIC_MACHINE.with(|cell| {
        if let Some(machine) = &mut *cell.borrow_mut() {
            // エラーを処理するが関数からは伝播させない
            if let Err(e) = machine.send(event) {
                console::log_1(&format!("Error sending event: {}", e).into());
            } else {
                success = true;
            }
        }
    });

    if success {
        update_music_player_state();
        Ok(())
    } else {
        Err(JsValue::from_str("Failed to send event"))
    }
}

fn update_music_player_state() {
    MUSIC_MACHINE.with(|cell| {
        if let Some(machine) = &*cell.borrow() {
            let states: Vec<&String> = machine.current_states.iter().collect();
            let states_json = serde_json::to_string(&states).unwrap_or_default();

            // JsValueに変換する前にクローンを作成
            let js_status = JsValue::from_str(&states_json);

            console::log_2(&"Current states:".into(), &js_status);

            // JS側から呼び出される関数を設定
            if let Some(update_fn) =
                js_sys::Reflect::get(&js_sys::global(), &"updateMusicPlayerUI".into()).ok()
            {
                if update_fn.is_function() {
                    let function = update_fn.dyn_into::<js_sys::Function>().unwrap();
                    let _ = js_sys::Reflect::apply(
                        &function,
                        &JsValue::NULL,
                        &js_sys::Array::of1(&JsValue::from_str(&states_json)),
                    );
                }
            }
        }
    });
}

// 交通信号機の状態機械を作成
fn create_traffic_light() -> crate::Result<Machine> {
    // 状態を作成
    let green = State::new("green");
    let yellow = State::new("yellow");
    let red = State::new("red");

    // トランジションを作成
    let green_to_yellow = Transition::new("green", "TIMER", "yellow");
    let yellow_to_red = Transition::new("yellow", "TIMER", "red");
    let red_to_green = Transition::new("red", "TIMER", "green");

    // アクションを定義
    let log_green = Action::new("logGreen", ActionType::Entry, |_ctx, _evt| {
        console::log_1(&"Entering GREEN state - Go!".into())
    });

    let log_yellow = Action::new("logYellow", ActionType::Entry, |_ctx, _evt| {
        console::log_1(&"Entering YELLOW state - Slow down!".into())
    });

    let log_red = Action::new("logRed", ActionType::Entry, |_ctx, _evt| {
        console::log_1(&"Entering RED state - Stop!".into())
    });

    // 機械を構築
    let machine = MachineBuilder::new("trafficLight")
        .state(green)
        .state(yellow)
        .state(red)
        .initial("green")
        .transition(green_to_yellow)
        .transition(yellow_to_red)
        .transition(red_to_green)
        .on_entry("green", log_green)
        .on_entry("yellow", log_yellow)
        .on_entry("red", log_red)
        .build()?;

    Ok(machine)
}

// 音楽プレーヤーの状態機械を作成
fn create_music_player() -> crate::Result<Machine> {
    // 状態を作成
    let power_off = State::new("powerOff");

    let mut player = State::new_compound("player", "stopped");
    player.parent = Some("root".to_string());

    let mut stopped = State::new("stopped");
    stopped.parent = Some("player".to_string());

    let mut playing = State::new_compound("playing", "normal");
    playing.parent = Some("player".to_string());

    let mut normal = State::new("normal");
    normal.parent = Some("playing".to_string());

    let mut double_speed = State::new("doubleSpeed");
    double_speed.parent = Some("playing".to_string());

    let mut paused = State::new("paused");
    paused.parent = Some("player".to_string());

    // トランジションを作成
    let power_toggle = Transition::new("powerOff", "POWER", "player");
    let power_off_transition = Transition::new("player", "POWER", "powerOff");

    let play = Transition::new("stopped", "PLAY", "playing");
    let stop = Transition::new("playing", "STOP", "stopped");
    let pause = Transition::new("playing", "PAUSE", "paused");
    let resume = Transition::new("paused", "PLAY", "playing");

    let speed_up = Transition::new("normal", "SPEED_UP", "doubleSpeed");
    let speed_normal = Transition::new("doubleSpeed", "SPEED_NORMAL", "normal");

    let next_track = Transition::internal_transition("playing", "NEXT");
    let prev_track = Transition::internal_transition("playing", "PREV");

    // アクションを作成
    let log_power_on = Action::new("logPowerOn", ActionType::Entry, |_ctx, _evt| {
        console::log_1(&"Power ON - Player ready".into())
    });

    let log_power_off = Action::new("logPowerOff", ActionType::Entry, |_ctx, _evt| {
        console::log_1(&"Power OFF".into())
    });

    let log_playing = Action::new("logPlaying", ActionType::Entry, |_ctx, _evt| {
        console::log_1(&"Playing track".into())
    });

    let next_track_action = Action::new("nextTrack", ActionType::Transition, |ctx, _evt| {
        let current_track = ctx.get::<usize>("track").unwrap_or(0);
        let next_track = current_track + 1;
        console::log_1(&format!("Changing to track {}", next_track).into());
        let _ = ctx.set("track", next_track);
    });

    let prev_track_action = Action::new("prevTrack", ActionType::Transition, |ctx, _evt| {
        let current_track = ctx.get::<usize>("track").unwrap_or(0);
        let prev_track = if current_track > 0 {
            current_track - 1
        } else {
            0
        };
        console::log_1(&format!("Changing to track {}", prev_track).into());
        let _ = ctx.set("track", prev_track);
    });

    // コンテキストを作成
    let mut context = Context::new();
    let _ = context.set("track", 0);

    // トランジションにアクションを設定
    let mut next_track = next_track;
    next_track.with_action(next_track_action);

    let mut prev_track = prev_track;
    prev_track.with_action(prev_track_action);

    // 状態機械を構築
    let machine = MachineBuilder::new("musicPlayer")
        .initial("powerOff")
        .state(power_off)
        .state(player)
        .state(stopped)
        .state(playing)
        .state(normal)
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
        .transition(next_track)
        .transition(prev_track)
        .on_entry("player", log_power_on)
        .on_entry("powerOff", log_power_off)
        .on_entry("playing", log_playing)
        .context(context)
        .build()?;

    Ok(machine)
}
