use rustate::{
    Action, ActionType, Context, Machine, MachineBuilder, State, Transition,
    #[cfg(feature = "codegen")]
    CodegenExt,
    #[cfg(feature = "codegen")]
    JsonExportOptions,
    #[cfg(feature = "proto")]
    ProtoExportOptions,
};

fn main() -> rustate::Result<()> {
    // オンラインショッピングのステートマシンを作成
    let mut machine = create_shopping_machine()?;
    println!("ステートマシンを作成しました: {}", machine.name);
    println!("現在の状態: {:?}", machine.current_states);

    // 初期状態のコンテキスト
    println!(
        "初期アイテム数: {:?}",
        machine.context.get::<i32>("itemCount")
    );

    // イベントを送信してステートマシンを実行
    println!("\nSTARTイベントを送信");
    machine.send("START")?;
    println!("現在の状態: {:?}", machine.current_states);

    println!("\nADD_ITEMイベントを送信");
    machine.send("ADD_ITEM")?;
    println!("現在の状態: {:?}", machine.current_states);

    // カートの商品数を取得
    let item_count = machine.context.get::<i32>("itemCount").unwrap_or(0);
    println!("カート内の商品数: {}", item_count);

    // カート内の商品数を手動で増やしてみる
    let _ = machine.context.set("itemCount", item_count + 5);
    println!(
        "手動で更新した後のカート内商品数: {}",
        machine.context.get::<i32>("itemCount").unwrap_or(0)
    );

    println!("\nCHECKOUTイベントを送信");
    machine.send("CHECKOUT")?;
    println!("現在の状態: {:?}", machine.current_states);

    println!("\nPAYイベントを送信");
    machine.send("PAY")?;
    println!("現在の状態: {:?}", machine.current_states);

    // 決済状態を確認
    let payment_processed = machine
        .context
        .get::<bool>("paymentProcessed")
        .unwrap_or(false);
    println!("決済処理完了: {}", payment_processed);

    // 決済状態を手動で変更
    let _ = machine.context.set("paymentProcessed", true);
    println!(
        "手動で更新した後の決済状態: {}",
        machine
            .context
            .get::<bool>("paymentProcessed")
            .unwrap_or(false)
    );

    println!("\nCONFIRMイベントを送信");
    machine.send("CONFIRM")?;
    println!("現在の状態: {:?}", machine.current_states);

    println!("\nNEW_ORDERイベントを送信");
    machine.send("NEW_ORDER")?;
    println!("現在の状態: {:?}", machine.current_states);

    // codegen 機能が有効な場合、JSON と Proto ファイルを生成
    #[cfg(feature = "codegen")]
    {
        println!("\nステートマシン定義を JSON としてエクスポート中...");
        let json_options = JsonExportOptions {
            output_path: "shopping_cart.json".to_string(),
            pretty: true,
            include_metadata: true,
        };
        machine.export_to_json(Some(json_options))?;
        println!("JSON ファイルが生成されました: shopping_cart.json");
    }

    #[cfg(feature = "proto")]
    {
        println!("\nステートマシン定義を Protocol Buffers としてエクスポート中...");
        let proto_options = ProtoExportOptions {
            output_path: "shopping_cart.proto".to_string(),
            package_name: "shopping".to_string(),
            message_name: "ShoppingCart".to_string(),
        };
        machine.export_to_proto(Some(proto_options))?;
        println!("Proto ファイルが生成されました: shopping_cart.proto");
    }

    // ソースコードからステートマシン定義をパースする例 (通常は別コマンドとして実装)
    #[cfg(feature = "codegen")]
    {
        println!("\nRust ソースコードからステートマシン定義をパースする例（コメントアウト状態）");
        // 注：この機能を使用するには、実装を完成させる必要があります
        // let parsed_machine = Machine::parse_from_rust_file("examples/xstate_test_example.rs")?;
        // println!("パースされたマシン名: {}", parsed_machine.name);
    }

    Ok(())
}

// ショッピングカートの状態マシンを作成
fn create_shopping_machine() -> rustate::Result<Machine> {
    // 状態の作成
    let idle = State::new("idle");
    let browsing = State::new("browsing");
    let cart = State::new("cart");
    let checkout = State::new("checkout");
    let payment = State::new("payment");
    let confirmed = State::new("confirmed");

    // 初期コンテキスト
    let mut context = Context::new();
    let _ = context.set("itemCount", 0);
    let _ = context.set("paymentProcessed", false);

    // アクションの作成
    let add_to_cart = Action::new("addToCart", ActionType::Entry, |ctx, _| {
        let item_count = ctx.get::<i32>("itemCount").unwrap_or(0);
        let _ = ctx.set("itemCount", item_count + 1);
        println!(
            "カートに商品を追加しました。現在の商品数: {}",
            item_count + 1
        );
    });

    let process_payment = Action::new("processPayment", ActionType::Entry, |ctx, _| {
        println!("決済処理中...");
        let _ = ctx.set("paymentProcessed", true);
        println!("決済処理が完了しました");
    });

    // 遷移の作成
    let start_browsing = Transition::new("idle", "START", "browsing");
    let add_item = Transition::new("browsing", "ADD_ITEM", "cart");
    let continue_shopping = Transition::new("cart", "CONTINUE", "browsing");
    let proceed_checkout = Transition::new("cart", "CHECKOUT", "checkout");
    let pay = Transition::new("checkout", "PAY", "payment");
    let confirm = Transition::new("payment", "CONFIRM", "confirmed");
    let new_order = Transition::new("confirmed", "NEW_ORDER", "idle");

    // マシンの構築
    let machine = MachineBuilder::new("shoppingCart")
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
        .on_entry("cart", add_to_cart)
        .on_entry("payment", process_payment)
        .context(context)
        .build()?;

    Ok(machine)
}
