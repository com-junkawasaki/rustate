use rustate::{
    Action, ActionType, Machine, MachineBuilder, State, Transition,
    XStateTestModel, XStateTestPlan, XStateTestPath, XStatePathSegment,
    create_test_model, execute_test_plan,
};

fn main() {
    // オンラインショッピングのステートマシンを作成
    let machine = create_shopping_machine();
    println!("ステートマシンを作成しました: {}", machine.name);

    // XStateスタイルのテストモデルを作成
    let mut model = create_test_model(machine);

    // アサーションを追加
    model.assert("cartNotEmpty", |m| {
        let item_count = m.context.get::<i32>("itemCount").unwrap_or(0);
        item_count > 0
    });

    model.assert("paymentComplete", |m| {
        m.context.get::<bool>("paymentProcessed").unwrap_or(false)
    });

    // アクターの実装を提供
    model.provide("paymentProcessor", |ctx, _| {
        println!("決済処理を実行中...");
        let _ = ctx.set("paymentProcessed", true);
        Ok(())
    });

    // テストプランを自動生成
    let generated_plan = model.generate_paths(5);
    println!("\n=== 自動生成されたテストプラン ===");
    println!("プラン名: {}", generated_plan.name);
    println!("パス数: {}", generated_plan.paths.len());

    // 手動でテストプランを作成
    let manual_plan = create_test_plan();
    println!("\n=== 手動作成されたテストプラン ===");
    println!("プラン名: {}", manual_plan.name);
    println!("パス数: {}", manual_plan.paths.len());

    // テストプランを実行
    println!("\n=== テスト実行結果 ===");
    match execute_test_plan(&mut model, &manual_plan) {
        Ok(result) => {
            println!("テスト成功: {}", result.success);
            println!("全パスカバー: {}", result.all_paths_covered);
            
            for (i, path_result) in result.path_results.iter().enumerate() {
                println!("\nパス {}: {} ({})", 
                    i + 1, 
                    path_result.path_name,
                    if path_result.success { "成功" } else { "失敗" }
                );
                
                if !path_result.failures.is_empty() {
                    println!("失敗:");
                    for failure in &path_result.failures {
                        println!("  - セグメント {}: {}", failure.segment_index, failure.error);
                    }
                }
            }
            
            // カバレッジを取得
            let coverage = model.get_coverage();
            println!("\n=== カバレッジ情報 ===");
            println!("状態カバレッジ: {:.1}%", coverage.state_coverage_percentage());
            println!("遷移カバレッジ: {:.1}%", coverage.transition_coverage_percentage());
        }
        Err(err) => {
            println!("テスト実行中にエラーが発生しました: {}", err);
        }
    }
}

// ショッピングカートの状態マシンを作成
fn create_shopping_machine() -> Machine {
    // 状態の作成
    let idle = State::new("idle");
    let browsing = State::new("browsing");
    let cart = State::new("cart");
    let checkout = State::new("checkout");
    let payment = State::new("payment");
    let confirmed = State::new("confirmed");

    // アクションの作成
    let add_to_cart = Action::new("addToCart", ActionType::Transition, |ctx, _| {
        let item_count = ctx.get::<i32>("itemCount").unwrap_or(0);
        let _ = ctx.set("itemCount", item_count + 1);
        println!("カートに商品を追加しました");
    });

    let process_payment = Action::new("processPayment", ActionType::Transition, |ctx, _| {
        println!("決済処理中...");
        let _ = ctx.set("paymentProcessed", true);
    });

    // 遷移の作成
    let start_browsing = Transition::new("idle", "START", "browsing");
    let add_item = Transition::new("browsing", "ADD_ITEM", "cart").with_action(add_to_cart);
    let continue_shopping = Transition::new("cart", "CONTINUE", "browsing");
    let proceed_checkout = Transition::new("cart", "CHECKOUT", "checkout");
    let pay = Transition::new("checkout", "PAY", "payment").with_action(process_payment);
    let confirm = Transition::new("payment", "CONFIRM", "confirmed");
    let new_order = Transition::new("confirmed", "NEW_ORDER", "idle");

    // マシンの構築
    MachineBuilder::new("shoppingCart")
        .state(idle)
        .state(browsing)
        .state(cart)
        .state(checkout)
        .state(payment)
        .state(confirmed)
        .initial("idle")
        .transition(start_browsing)
        .transition(add_item)
        .transition(continue_shopping)
        .transition(proceed_checkout)
        .transition(pay)
        .transition(confirm)
        .transition(new_order)
        .build()
        .unwrap()
}

// 手動でテストプランを作成
fn create_test_plan() -> XStateTestPlan {
    XStateTestPlan {
        name: "Shopping Cart Test Plan".to_string(),
        description: Some("オンラインショッピングカートの全フローをテスト".to_string()),
        preconditions: None,
        paths: vec![
            // 通常のショッピングフロー
            XStateTestPath {
                name: "Complete Shopping Flow".to_string(),
                description: Some("ブラウジングから注文確定までの正常なフロー".to_string()),
                segments: vec![
                    XStatePathSegment {
                        state: "idle".to_string(),
                        event: Some("START".to_string()),
                        assertions: None,
                    },
                    XStatePathSegment {
                        state: "browsing".to_string(),
                        event: Some("ADD_ITEM".to_string()),
                        assertions: None,
                    },
                    XStatePathSegment {
                        state: "cart".to_string(),
                        event: None,
                        assertions: Some(vec!["cartNotEmpty".to_string()]),
                    },
                    XStatePathSegment {
                        state: "cart".to_string(),
                        event: Some("CHECKOUT".to_string()),
                        assertions: None,
                    },
                    XStatePathSegment {
                        state: "checkout".to_string(),
                        event: Some("PAY".to_string()),
                        assertions: None,
                    },
                    XStatePathSegment {
                        state: "payment".to_string(),
                        event: None,
                        assertions: Some(vec!["paymentComplete".to_string()]),
                    },
                    XStatePathSegment {
                        state: "payment".to_string(),
                        event: Some("CONFIRM".to_string()),
                        assertions: None,
                    },
                    XStatePathSegment {
                        state: "confirmed".to_string(),
                        event: None,
                        assertions: None,
                    },
                ],
            },
            // 商品追加後に買い物を続けるフロー
            XStateTestPath {
                name: "Continue Shopping Flow".to_string(),
                description: Some("カートに商品を追加後、ショッピングを継続するフロー".to_string()),
                segments: vec![
                    XStatePathSegment {
                        state: "idle".to_string(),
                        event: Some("START".to_string()),
                        assertions: None,
                    },
                    XStatePathSegment {
                        state: "browsing".to_string(),
                        event: Some("ADD_ITEM".to_string()),
                        assertions: None,
                    },
                    XStatePathSegment {
                        state: "cart".to_string(),
                        event: None,
                        assertions: Some(vec!["cartNotEmpty".to_string()]),
                    },
                    XStatePathSegment {
                        state: "cart".to_string(),
                        event: Some("CONTINUE".to_string()),
                        assertions: None,
                    },
                    XStatePathSegment {
                        state: "browsing".to_string(),
                        event: Some("ADD_ITEM".to_string()),
                        assertions: None,
                    },
                    XStatePathSegment {
                        state: "cart".to_string(),
                        event: Some("CHECKOUT".to_string()),
                        assertions: None,
                    },
                    XStatePathSegment {
                        state: "checkout".to_string(),
                        event: Some("PAY".to_string()),
                        assertions: None,
                    },
                    XStatePathSegment {
                        state: "payment".to_string(),
                        event: Some("CONFIRM".to_string()),
                        assertions: None,
                    },
                    XStatePathSegment {
                        state: "confirmed".to_string(),
                        event: None,
                        assertions: None,
                    },
                ],
            },
        ],
    }
} 