use rustate::{
    transition::TransitionType, Action, Context, Event, EventTrait, IntoEvent, Machine,
    MachineBuilder, State, Transition,
};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::sync::Arc;
use tokio::sync::RwLock;

#[cfg(feature = "codegen")]
use rustate::{CodegenExt, JsonExportOptions};

#[cfg(feature = "proto")]
use rustate::ProtoExportOptions;

use std::env;

// Define the event enum
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
enum ShoppingEvent {
    Start,
    AddItem,
    Continue,
    Checkout,
    Pay,
    Confirm,
    NewOrder,
    #[default]
    None,
}

// Implement EventTrait for the enum
impl EventTrait for ShoppingEvent {
    fn name(&self) -> &str {
        match self {
            ShoppingEvent::Start => "Start",
            ShoppingEvent::AddItem => "AddItem",
            ShoppingEvent::Continue => "Continue",
            ShoppingEvent::Checkout => "Checkout",
            ShoppingEvent::Pay => "Pay",
            ShoppingEvent::Confirm => "Confirm",
            ShoppingEvent::NewOrder => "NewOrder",
            ShoppingEvent::None => "None",
        }
    }
    fn event_type(&self) -> &str {
        self.name()
    }
    fn payload(&self) -> Option<&serde_json::Value> {
        None
    }
}

// Implement IntoEvent for ShoppingEvent
impl IntoEvent for ShoppingEvent {
    fn into_event(self) -> Event {
        Event::new(self.name())
    }
}

impl fmt::Display for ShoppingEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

#[tokio::main]
async fn main() -> rustate::Result<()> {
    // 現在の実行ディレクトリを取得
    let current_dir = env::current_dir().expect("カレントディレクトリの取得に失敗しました");
    println!("現在のディレクトリ: {}", current_dir.display());

    // オンラインショッピングのステートマシンを作成
    let mut machine: Machine<Context, ShoppingEvent, String, ()> =
        create_shopping_machine().await?;
    println!("ステートマシンを作成しました: {}", machine.name);
    println!("現在の状態: {:?}", machine.current_states);

    // 初期状態のコンテキスト
    println!(
        "初期アイテム数: {:?}",
        machine.context.read().await.get::<i32>("itemCount")
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
        machine.context.read().await.get::<i32>("itemCount")
    );

    println!("\nCheckoutイベントを送信");
    machine.send(ShoppingEvent::Checkout).await?;
    println!("現在の状態: {:?}", machine.current_states);

    println!("\nPayイベントを送信");
    machine.send(ShoppingEvent::Pay).await?;
    println!("現在の状態: {:?}", machine.current_states);
    println!(
        "支払い処理済み: {:?}",
        machine.context.read().await.get::<bool>("paymentProcessed")
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
async fn create_shopping_machine() -> rustate::Result<Machine<Context, ShoppingEvent, String, ()>> {
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

    // Define actions
    let process_payment_action = Action::from_fn(|ctx_arc: Arc<RwLock<Context>>, _evt| {
        Box::pin(async move {
            println!("ACTION: Processing payment...");
            let _ = ctx_arc.write().await.set("paymentProcessed", true);
            Ok(())
        })
    });
    let add_to_cart_action = Action::from_fn(|ctx_arc: Arc<RwLock<Context>>, _evt| {
        Box::pin(async move {
            let item_count_result = ctx_arc.read().await.get::<i32>("itemCount");
            let item_count = item_count_result.and_then(|res| res.ok()).unwrap_or(0);
            let _ = ctx_arc.write().await.set("itemCount", item_count + 1);
            println!("ACTION: Added item to cart. Total items: {}", item_count + 1);
            Ok(())
        })
    });

    // 遷移の作成
    let start_browsing = Transition::new(
        idle.clone(),
        Some(browsing.clone()),
        Some(ShoppingEvent::Start),
        None,
        vec![],
        TransitionType::External,
    );
    let add_item = Transition::new(
        browsing.clone(),
        Some(cart.clone()),
        Some(ShoppingEvent::AddItem),
        None,
        vec![add_to_cart_action.clone()],
        TransitionType::External,
    );
    let continue_shopping = Transition::new(
        cart.clone(),
        Some(browsing.clone()),
        Some(ShoppingEvent::Continue),
        None,
        vec![],
        TransitionType::External,
    );
    let proceed_checkout = Transition::new(
        cart.clone(),
        Some(checkout.clone()),
        Some(ShoppingEvent::Checkout),
        None,
        vec![],
        TransitionType::External,
    );
    let pay = Transition::new(
        checkout.clone(),
        Some(payment.clone()),
        Some(ShoppingEvent::Pay),
        None,
        vec![process_payment_action.clone()],
        TransitionType::External,
    );
    let confirm = Transition::new(
        payment.clone(),
        Some(confirmed.clone()),
        Some(ShoppingEvent::Confirm),
        None,
        vec![],
        TransitionType::External,
    );
    let new_order = Transition::new(
        confirmed.clone(),
        Some(idle.clone()),
        Some(ShoppingEvent::NewOrder),
        None,
        vec![],
        TransitionType::External,
    );

    // マシンの構築
    let machine = MachineBuilder::new("shoppingCart".to_string(), idle.clone())
        .state(State::new(idle.clone()))
        .state(State::new(browsing.clone()))
        .state(State::new(cart.clone()))
        .state(State::new(checkout.clone()))
        .state(State::new(payment.clone()))
        .state(State::new(confirmed.clone()))
        .transition(start_browsing)
        .transition(add_item)
        .transition(continue_shopping)
        .transition(proceed_checkout)
        .transition(pay)
        .transition(confirm)
        .transition(new_order)
        .context(context)
        .build()
        .await?;

    Ok(machine)
}
