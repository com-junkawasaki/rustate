use rustate::machine::Machine;
use serde_json::json;
use std::collections::HashMap;

// ステートマシンのレイアウトを自動生成するためのユーティリティ
#[allow(dead_code)]
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
            // Set position in data field
            let position_data = json!({
                "x": x,
                "y": y
            });

            state.with_data(position_data);
            visited.insert(state_id.clone(), (x, y));

            // 次の位置を計算
            x += padding;
            if x > 600.0 {
                x = 100.0;
                y += padding;
            }
        }

        // このステートからの遷移先を取得
        let targets: Vec<String> = machine
            .transitions
            .iter()
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
            // Set position in data field
            let position_data = json!({
                "x": x,
                "y": y
            });

            state.with_data(position_data);

            x += padding;
            if x > 600.0 {
                x = 100.0;
                y += padding;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wasm_bindgen_test::*;

    wasm_bindgen_test_configure!(run_in_browser);

    // モックDOM要素を作成するヘルパー
    fn create_mock_element() -> web_sys::Element {
        let window = web_sys::window().expect("no global window exists");
        let document = window.document().expect("no document exists");
        document.create_element("div").expect("could not create element")
    }

    #[wasm_bindgen_test]
    fn test_layout_creation() {
        let layout = Layout::new("test-layout");
        
        assert_eq!(layout.id, "test-layout");
        assert_eq!(layout.regions.len(), 0);
    }

    #[wasm_bindgen_test]
    fn test_add_region() {
        let mut layout = Layout::new("test-layout");
        
        // 領域を追加
        layout.add_region("sidebar", RegionType::Sidebar, 25.0);
        layout.add_region("main", RegionType::Content, 75.0);
        
        // 追加された領域の確認
        assert_eq!(layout.regions.len(), 2);
        
        // サイドバー領域の確認
        let sidebar = layout.regions.get("sidebar").unwrap();
        assert_eq!(sidebar.id, "sidebar");
        assert_eq!(sidebar.region_type, RegionType::Sidebar);
        assert_eq!(sidebar.size, 25.0);
        
        // メイン領域の確認
        let main = layout.regions.get("main").unwrap();
        assert_eq!(main.id, "main");
        assert_eq!(main.region_type, RegionType::Content);
        assert_eq!(main.size, 75.0);
    }

    #[wasm_bindgen_test]
    fn test_remove_region() {
        let mut layout = Layout::new("test-layout");
        
        // 領域を追加
        layout.add_region("sidebar", RegionType::Sidebar, 25.0);
        layout.add_region("main", RegionType::Content, 75.0);
        
        // 追加確認
        assert_eq!(layout.regions.len(), 2);
        
        // 領域を削除
        layout.remove_region("sidebar");
        
        // 削除後の確認
        assert_eq!(layout.regions.len(), 1);
        assert!(!layout.regions.contains_key("sidebar"));
        assert!(layout.regions.contains_key("main"));
    }

    #[wasm_bindgen_test]
    fn test_update_region_size() {
        let mut layout = Layout::new("test-layout");
        
        // 領域を追加
        layout.add_region("sidebar", RegionType::Sidebar, 25.0);
        
        // サイズを更新
        layout.update_region_size("sidebar", 30.0);
        
        // 更新確認
        let sidebar = layout.regions.get("sidebar").unwrap();
        assert_eq!(sidebar.size, 30.0);
    }

    #[wasm_bindgen_test]
    fn test_layout_render() {
        let mut layout = Layout::new("test-layout");
        
        // 領域を追加
        layout.add_region("sidebar", RegionType::Sidebar, 25.0);
        layout.add_region("main", RegionType::Content, 75.0);
        
        // コンテナ要素を作成
        let container = create_mock_element();
        
        // レイアウトをレンダリング
        let result = layout.render(&container);
        
        // エラーが発生しないことを確認
        assert!(result.is_ok());
    }

    #[wasm_bindgen_test]
    fn test_get_region_element() {
        let mut layout = Layout::new("test-layout");
        
        // 領域を追加
        layout.add_region("sidebar", RegionType::Sidebar, 25.0);
        
        // コンテナ要素を作成
        let container = create_mock_element();
        
        // レイアウトをレンダリング
        let _ = layout.render(&container);
        
        // 領域要素の取得を試みる（実際にはレンダリングの制限があるためnull）
        let element = layout.get_region_element("sidebar");
        
        // テスト環境では実際のDOM要素は取得できないため、Noneであることを確認
        assert!(element.is_none());
    }

    #[wasm_bindgen_test]
    fn test_region_type_serialization() {
        // 各RegionTypeの文字列表現を確認
        assert_eq!(RegionType::Sidebar.to_string(), "sidebar");
        assert_eq!(RegionType::Content.to_string(), "content");
        assert_eq!(RegionType::Toolbar.to_string(), "toolbar");
        assert_eq!(RegionType::Properties.to_string(), "properties");
    }
}
