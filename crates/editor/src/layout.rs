use rustate::machine::Machine;
use serde_json::json;
use std::collections::HashMap;

// ステートマシンのレイアウトを自動生成するためのユーティリティ
pub fn auto_layout(machine: &mut Machine) {
    let mut x = 100.0;
    let mut y = 100.0;
    let padding = 150.0;
    
    let mut visited = HashMap::new();
    let mut queue = Vec::new();
    
    // 初期ステートを見つける
    if !machine.initial.is_empty() {
        queue.push(machine.initial.clone());
    } else if !machine.states.is_empty() {
        // 初期ステートがなければ、最初のステートを使う
        queue.push(machine.states.keys().next().unwrap().clone());
    }
    
    // 幅優先探索でレイアウト
    while let Some(state_id) = queue.pop() {
        if visited.contains_key(&state_id) {
            continue;
        }
        
        // このステートの位置を設定
        if let Some(state) = machine.states.get_mut(&state_id) {
            // Create metadata if it doesn't exist
            if !state.metadata.is_object() {
                state.metadata = serde_json::Value::Object(serde_json::Map::new());
            }
            
            // Set x and y values
            if let Some(obj) = state.metadata.as_object_mut() {
                obj.insert("x".to_string(), json!(x));
                obj.insert("y".to_string(), json!(y));
            }
            
            visited.insert(state_id.clone(), (x, y));
            
            // 次の位置を計算
            x += padding;
            if x > 600.0 {
                x = 100.0;
                y += padding;
            }
        }
        
        // このステートからの遷移先を取得
        let targets: Vec<String> = machine.transitions.iter()
            .filter(|t| t.source == state_id)
            .filter_map(|t| t.target.clone())
            .collect();
        
        // 遷移先をキューに追加
        for target in targets {
            if !visited.contains_key(&target) {
                queue.push(target);
            }
        }
    }
    
    // 残ったステートを処理（孤立したステート）
    for (id, state) in &mut machine.states {
        if !visited.contains_key(id) {
            // Create metadata if it doesn't exist
            if !state.metadata.is_object() {
                state.metadata = serde_json::Value::Object(serde_json::Map::new());
            }
            
            // Set x and y values
            if let Some(obj) = state.metadata.as_object_mut() {
                obj.insert("x".to_string(), json!(x));
                obj.insert("y".to_string(), json!(y));
            }
            
            x += padding;
            if x > 600.0 {
                x = 100.0;
                y += padding;
            }
        }
    }
}