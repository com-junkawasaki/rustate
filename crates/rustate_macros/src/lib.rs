extern crate proc_macro;

use proc_macro::TokenStream;
use quote::quote;
// use syn::{parse_macro_input, DeriveInput}; // 一時的にコメントアウト

/// XState v5 ライクな定義から ActorLogic を実装する構造体を生成する（予定の）マクロ。
/// 現時点ではダミーの実装です。
#[proc_macro]
pub fn create_machine(input: TokenStream) -> TokenStream {
    // 入力トークンストリームをデバッグ出力
    println!("create_machine input tokens:\n{:#?}", input);

    // 本来はここで input を syn を使って解析し、
    // 状態、イベント、コンテキスト、遷移などの情報を抽出する。
    // 例: let parsed_input = parse_macro_input!(input as YourMachineDefinitionParser);

    // 解析結果に基づいてコードを生成する (quote!)
    // ここではダミーの構造体と ActorLogic の空実装を生成する

    // TODO: ここで入力に基づいて名前などを決定する
    let machine_name = syn::Ident::new("MyGeneratedMachineLogic", proc_macro2::Span::call_site());
    let context_type = syn::Ident::new("MyContext", proc_macro2::Span::call_site());
    let event_type = syn::Ident::new("MyEvent", proc_macro2::Span::call_site());
    let state_type = syn::Ident::new("MyState", proc_macro2::Span::call_site());

    let expanded = quote! {
        // ダミーの型定義 (本来はマクロ入力から導出するか、既存の型を使う)
        #[derive(Debug, Clone, Default, PartialEq, Eq, ::serde::Serialize, ::serde::Deserialize)]
        pub struct #context_type { /* ... context fields ... */ pub value: i32 }
        #[derive(Debug, Clone, PartialEq, Eq, ::serde::Serialize, ::serde::Deserialize)]
        pub enum #event_type { /* ... event variants ... */ DummyEvent }
        #[derive(Debug, Clone, PartialEq, Eq, ::serde::Serialize, ::serde::Deserialize)]
        pub enum #state_type { /* ... state variants ... */ Initial }

        // 生成されるステートマシンロジック構造体
        #[derive(Debug, Clone, Default)]
        pub struct #machine_name;

        // ActorLogic トレイトの実装 (ダミー)
        #[::async_trait::async_trait]
        impl ::rustate_core::logic::ActorLogic for #machine_name {
            type Context = #context_type;
            type Event = #event_type;
            type State = #state_type;

            fn initial(&self) -> (Self::State, Self::Context) {
                println!("WARN: Using dummy initial state/context from generated macro.");
                (#state_type::Initial, #context_type::default())
            }

            async fn transition(
                &self,
                state: Self::State,
                context: Self::Context,
                event: Self::Event,
            ) -> Result<(Self::State, Self::Context), ::rustate_core::actor::ActorError> {
                println!("WARN: Using dummy transition logic from generated macro. State: {:?}, Context: {:?}, Event: {:?}", state, context, event);
                // 何もせず現在の状態とコンテキストを返す（またはエラー）
                Ok((state, context))
                // Err(::rustate_core::actor::ActorError::UnexpectedEvent)
            }
        }

        // 必要であれば、Actor トレイトを実装するラッパー構造体も生成するかもしれない
        /*
        #[derive(Debug, Clone, Default)]
        pub struct MyGeneratedActor {
            logic: #machine_name,
        }

        #[::async_trait::async_trait]
        impl ::rustate_core::actor::Actor for MyGeneratedActor {
            type State = #context_type; // Actor の State は Context になることが多い
            type Event = #event_type;
            type Output = (); // またはマクロで定義可能に

            fn initial_state(&self) -> Self::State {
                self.logic.initial().1 // Context を返す
            }

            async fn receive(
                &self,
                state: Self::State, // ここでの state は Context
                event: Self::Event,
            ) -> Result<Self::State, ::rustate_core::actor::ActorError> {
                // ここで ActorLogic の transition を呼び出す必要があるが、
                // ActorLogic は内部状態 (State enum) を持つので、工夫が必要。
                // Actor 自身が State enum も保持する必要があるかもしれない。
                // このあたりの設計は要検討。一旦 ActorLogic 実装の生成に集中する。
                println!("WARN: Actor::receive not fully implemented in generated code.");
                Ok(state)
            }
        }
        */
    };

    // 生成されたコードをトークンストリームとして返す
    TokenStream::from(expanded)
} 