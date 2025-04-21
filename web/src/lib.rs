use wasm_bindgen::prelude::*;
use rustate::{Machine, State, Transition};

// When the `wee_alloc` feature is enabled, use `wee_alloc` as the global
// allocator.
#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

// Called when the wasm module is instantiated
#[wasm_bindgen(start)]
pub fn start() -> Result<(), JsValue> {
    // Setup panic hook for better error messages
    console_error_panic_hook::set_once();
    
    // Log a message to the console
    web_sys::console::log_1(&"RuState WebAssembly library initialized!".into());
    Ok(())
}

// Re-export key rustate types
#[wasm_bindgen]
pub fn init() {
    console_error_panic_hook::set_once();
    web_sys::console::log_1(&"RuState module initialized".into());
}

// Traffic Light Machine
#[wasm_bindgen]
pub fn init_traffic_light() -> Result<(), JsValue> {
    web_sys::console::log_1(&"Initializing traffic light state machine".into());
    
    // Here you'd create the actual state machine
    // For now just simulate by updating the UI
    let window = web_sys::window().expect("no global window exists");
    let update_function = js_sys::Reflect::get(
        &window,
        &JsValue::from_str("updateTrafficLightUI")
    )?;
    
    if js_sys::Reflect::has(&update_function, &JsValue::from_str("call"))? {
        let _ = js_sys::Reflect::apply(
            &update_function,
            &window,
            &js_sys::Array::of1(&JsValue::from_str("green"))
        )?;
    }
    
    Ok(())
}

#[wasm_bindgen]
pub fn send_traffic_light_event(event_type: &str) -> Result<(), JsValue> {
    web_sys::console::log_1(&format!("Traffic light received event: {}", event_type).into());
    
    // In a real implementation, you'd update the state machine
    // For now just simulate by cycling through states
    let state = match event_type {
        "TIMER" => {
            // Simulate cycling through states
            let window = web_sys::window().expect("no global window exists");
            let document = window.document().expect("should have a document on window");
            
            let state_element = document.get_element_by_id("traffic-state")
                .expect("should have state element");
            
            let current_state = state_element.text_content()
                .expect("should have text content");
            
            match current_state.as_str() {
                "green" => "yellow",
                "yellow" => "red",
                "red" => "green",
                _ => "green"
            }
        },
        _ => "green"
    };
    
    // Update UI
    let window = web_sys::window().expect("no global window exists");
    let update_function = js_sys::Reflect::get(
        &window,
        &JsValue::from_str("updateTrafficLightUI")
    )?;
    
    if js_sys::Reflect::has(&update_function, &JsValue::from_str("call"))? {
        let _ = js_sys::Reflect::apply(
            &update_function,
            &window,
            &js_sys::Array::of1(&JsValue::from_str(state))
        )?;
    }
    
    Ok(())
}

// Music Player Machine
#[wasm_bindgen]
pub fn init_music_player() -> Result<(), JsValue> {
    web_sys::console::log_1(&"Initializing music player state machine".into());
    
    // Here you'd create the actual state machine
    // For now just simulate by updating the UI
    let window = web_sys::window().expect("no global window exists");
    let update_function = js_sys::Reflect::get(
        &window,
        &JsValue::from_str("updateMusicPlayerUI")
    )?;
    
    if js_sys::Reflect::has(&update_function, &JsValue::from_str("call"))? {
        // Initial state: powerOff
        let states_json = serde_json::to_string(&vec!["powerOff"]).unwrap();
        let _ = js_sys::Reflect::apply(
            &update_function,
            &window,
            &js_sys::Array::of1(&JsValue::from_str(&states_json))
        )?;
    }
    
    Ok(())
}

#[wasm_bindgen]
pub fn send_music_player_event(event_type: &str) -> Result<(), JsValue> {
    web_sys::console::log_1(&format!("Music player received event: {}", event_type).into());
    
    // In a real implementation, you'd update the state machine
    // For now just simulate state changes based on event
    let window = web_sys::window().expect("no global window exists");
    let document = window.document().expect("should have a document on window");
    
    let status_element = document.get_element_by_id("player-status")
        .expect("should have player status element");
    
    let current_status = status_element.text_content()
        .expect("should have text content");
    
    // Extract current states from status text
    let status_text = current_status.trim_start_matches("Status: ");
    let current_states: Vec<&str> = status_text.split(", ").collect();
    
    // Determine new states based on event
    let new_states = match event_type {
        "POWER" => {
            if current_states.contains(&"powerOff") {
                vec!["on", "stopped", "normal"]
            } else {
                vec!["powerOff"]
            }
        },
        "PLAY" => {
            if !current_states.contains(&"powerOff") && !current_states.contains(&"playing") {
                vec!["on", "playing", "normal"]
            } else {
                current_states.iter().map(|&s| s).collect()
            }
        },
        "PAUSE" => {
            if current_states.contains(&"playing") {
                vec!["on", "paused", "normal"]
            } else {
                current_states.iter().map(|&s| s).collect()
            }
        },
        "STOP" => {
            if !current_states.contains(&"powerOff") {
                vec!["on", "stopped", "normal"]
            } else {
                current_states.iter().map(|&s| s).collect()
            }
        },
        "SPEED_UP" => {
            if current_states.contains(&"playing") && current_states.contains(&"normal") {
                vec!["on", "playing", "fast"]
            } else {
                current_states.iter().map(|&s| s).collect()
            }
        },
        "SPEED_NORMAL" => {
            if current_states.contains(&"playing") && !current_states.contains(&"normal") {
                vec!["on", "playing", "normal"]
            } else {
                current_states.iter().map(|&s| s).collect()
            }
        },
        _ => current_states.iter().map(|&s| s).collect()
    };
    
    // Convert states to strings for JavaScript
    let states: Vec<String> = new_states.iter().map(|&s| s.to_string()).collect();
    let states_json = serde_json::to_string(&states).unwrap();
    
    // Update UI
    let update_function = js_sys::Reflect::get(
        &window,
        &JsValue::from_str("updateMusicPlayerUI")
    )?;
    
    if js_sys::Reflect::has(&update_function, &JsValue::from_str("call"))? {
        let _ = js_sys::Reflect::apply(
            &update_function,
            &window,
            &js_sys::Array::of1(&JsValue::from_str(&states_json))
        )?;
    }
    
    Ok(())
} 