use rustate::machine::StateMachine;
use serde_json::json;
use std::collections::HashMap;

// ステートマシンのレイアウトを自動生成するためのユーティリティ
pub fn auto_layout(machine: &mut StateMachine) {
    let mut x = 100.0;
    let mut y = 100.0;
    let padding = 150.0;
    
    let mut visited = HashMap::new();
    let mut queue = Vec::new();
    
    // 初期ステートを見つける
    let initial_state = machine.states.iter()
        .find(|(_, state)| state.initial)
        .map(|(id, _)| id.clone());
    
    if let Some(initial) = initial_state {
        queue.push(initial);
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
            state.metadata = json!({
                "x": x,
                "y": y
            });
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
            .filter(|(_, t)| t.source == state_id)
            .map(|(_, t)| t.target.clone())
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
            state.metadata = json!({
                "x": x,
                "y": y
            });
            
            x += padding;
            if x > 600.0 {
                x = 100.0;
                y += padding;
            }
        }
    }
}