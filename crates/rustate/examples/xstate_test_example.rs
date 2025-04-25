use rustate::{
    Action, Context, Event, EventTrait, Machine, MachineBuilder, State, StateTrait, Transition,
    TransitionType,
};
use std::{any::Any, fmt};

#[cfg(feature = "codegen")]
use rustate::{CodegenExt, JsonExportOptions};

#[cfg(feature = "proto")]
use rustate::ProtoExportOptions;

use std::env;

// Define the event enum
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum ShoppingEvent {
    Start,
    AddItem,
    Continue,
    Checkout,
    Pay,
    Confirm,
    NewOrder,
}

// Implement EventTrait for the enum
impl EventTrait for ShoppingEvent {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn box_clone(&self) -> Box<dyn EventTrait> {
        Box::new(self.clone())
    }
    fn as_string(&self) -> String {
        format!("{:?}", self)
    }
}

impl fmt::Display for ShoppingEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_string())
    }
}

#[tokio::main]
async fn main() -> rustate::Result<()> {
    // 現在の実行ディレクトリを取得
    let current_dir = env::current_dir().expect("カレントディレクトリの取得に失敗しました");
    println!("現在のディレクトリ: {}", current_dir.display());

    // オンラインショッピングのステートマシンを作成
    let mut machine: Machine<String, ShoppingEvent, Context> =
        create_shopping_machine().await?;
    println!("ステートマシンを作成しました: {}", machine.name);
    println!("現在の状態: {:?}", machine.current_states);

    // 初期状態のコンテキスト
    println!(
        "初期アイテム数: {:?}",
        machine.context.get::<i32>("itemCount")
    );

    // イベントを送信してステートマシンを実行
    println!("\nStartイベントを送信");
    machine.send(ShoppingEvent::Start).await?;
    println!("現在の状態: {:?}", machine.current_states);

    println!("\nAddItemイベントを送信");
    machine.send(ShoppingEvent::AddItem).await?;
    println!("現在の状態: {:?}", machine.current_states);
    println!(
        "アイテム数: {:?}",
        machine.context.get::<i32>("itemCount")
    );

    println!("\nCheckoutイベントを送信");
    machine.send(ShoppingEvent::Checkout).await?;
    println!("現在の状態: {:?}", machine.current_states);

    println!("\nPayイベントを送信");
    machine.send(ShoppingEvent::Pay).await?;
    println!("現在の状態: {:?}", machine.current_states);
    println!(
        "支払い処理済み: {:?}",
        machine.context.get::<bool>("paymentProcessed")
    );

    println!("\nConfirmイベントを送信");
    machine.send(ShoppingEvent::Confirm).await?;
    println!("現在の状態: {:?}", machine.current_states);

    // codegen 機能が有効な場合、JSON ファイルを生成
    #[cfg(feature = "codegen")]
    {
        println!("\nステートマシン定義を JSON としてエクスポート中...");
        let json_path = current_dir.join("shopping_cart.json");
        let json_options = JsonExportOptions {
            output_path: json_path.to_string_lossy().to_string(),
            pretty: true,
            include_metadata: true,
        };
        machine.export_to_json(Some(json_options)).await?;
        println!("JSON ファイルが生成されました: {}", json_path.display());
    }

    #[cfg(feature = "proto")]
    {
        println!("\nステートマシン定義を Protocol Buffers としてエクスポート中...");
        let proto_path = current_dir.join("shopping_cart.proto");
        let proto_options = ProtoExportOptions {
            output_path: proto_path.to_string_lossy().to_string(),
            package_name: "shopping".to_string(),
            message_name: "ShoppingCart".to_string(),
        };
        machine.export_to_proto(Some(proto_options)).await?;
        println!("Proto ファイルが生成されました: {}", proto_path.display());
    }

    Ok(())
}

// ショッピングカートの状態マシンを作成
async fn create_shopping_machine() -> rustate::Result<Machine<String, ShoppingEvent, Context>> {
    // 状態 ID (String implements StateTrait)
    let idle = "idle".to_string();
    let browsing = "browsing".to_string();
    let cart = "cart".to_string();
    let checkout = "checkout".to_string();
    let payment = "payment".to_string();
    let confirmed = "confirmed".to_string();

    // 初期コンテキスト
    let mut context = Context::new();
    let _ = context.set("itemCount", 0i32); // Specify type for clarity
    let _ = context.set("paymentProcessed", false);

    // アクションの作成
    let add_to_cart_action = Action::from_fn("addToCart".to_string(), |mut ctx, _| async move {
        let item_count = ctx.get::<i32>("itemCount").unwrap_or(0);
        let new_count = item_count + 1;
        let _ = ctx.set("itemCount", new_count);
        println!(
            "カートに商品を追加しました。現在の商品数: {}",
            new_count
        );
    });

    let process_payment_action =
        Action::from_fn("processPayment".to_string(), |mut ctx, _| async move {
            println!("決済処理中...");
            let _ = ctx.set("paymentProcessed", true);
            println!("決済処理が完了しました");
        });

    // 遷移の作成
    let start_browsing = Transition::new(
        idle.clone(),
        ShoppingEvent::Start,
        browsing.clone(),
        None, // No guard
        vec![], // No actions
        TransitionType::External,
    );
    let add_item = Transition::new(
        browsing.clone(),
        ShoppingEvent::AddItem,
        cart.clone(),
        None,
        vec![add_to_cart_action.clone()], // Action on transition
        TransitionType::External,
    );
    let continue_shopping = Transition::new(
        cart.clone(),
        ShoppingEvent::Continue,
        browsing.clone(),
        None,
        vec![],
        TransitionType::External,
    );
    let proceed_checkout = Transition::new(
        cart.clone(),
        ShoppingEvent::Checkout,
        checkout.clone(),
        None,
        vec![],
        TransitionType::External,
    );
    let pay = Transition::new(
        checkout.clone(),
        ShoppingEvent::Pay,
        payment.clone(),
        None,
        vec![process_payment_action.clone()], // Action on transition
        TransitionType::External,
    );
    let confirm = Transition::new(
        payment.clone(),
        ShoppingEvent::Confirm,
        confirmed.clone(),
        None,
        vec![],
        TransitionType::External,
    );
    let new_order = Transition::new(
        confirmed.clone(),
        ShoppingEvent::NewOrder,
        idle.clone(),
        None,
        vec![], // Reset actions could go here if needed
        TransitionType::External,
    );

    // マシンの構築
    let machine = MachineBuilder::new("shoppingCart".to_string(), idle.clone())
        .states(vec![
            idle.clone(),
            browsing.clone(),
            cart.clone(),
            checkout.clone(),
            payment.clone(),
            confirmed.clone(),
        ])
        .transitions(vec![
            start_browsing,
            add_item,
            continue_shopping,
            proceed_checkout,
            pay,
            confirm,
            new_order,
        ])
        .context(context)
        .build()
        .await?;

    Ok(machine)
}
