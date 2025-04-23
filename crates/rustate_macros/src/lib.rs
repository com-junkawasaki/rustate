extern crate proc_macro;

use proc_macro::TokenStream;
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::{parse_macro_input, Ident, Token, Type, Expr, Result, braced, FieldValue, Path};

// Represents `FieldName: FieldValue` used in initial context
struct InitialContextField {
    member: Ident,
    expr: Expr,
}

impl Parse for InitialContextField {
    fn parse(input: ParseStream) -> Result<Self> {
        Ok(InitialContextField {
            member: input.parse()?,
            expr: {
                input.parse::<Token![:]>()?;
                input.parse()?
            },
        })
    }
}

// Represents the overall machine definition input
struct MachineDefinition {
    machine_name: Ident,
    context_type: Type,
    event_type: Type,
    state_type: Type,
    initial_state_variant: Path, // e.g., MySimpleState::Idle
    initial_context_fields: Punctuated<InitialContextField, Token![,]>;
    // TODO: Add fields for states, transitions, actions, guards etc. later
}

impl Parse for MachineDefinition {
    fn parse(input: ParseStream) -> Result<Self> {
        // 1. Parse Machine Name
        let machine_name: Ident = input.parse()?;
        input.parse::<Token![,]>()?;

        // 2. Parse fields like Context = Type, Event = Type, State = Type, initial: ...
        let mut context_type: Option<Type> = None;
        let mut event_type: Option<Type> = None;
        let mut state_type: Option<Type> = None;
        let mut initial_state_variant: Option<Path> = None;
        let mut initial_context_fields: Option<Punctuated<InitialContextField, Token![,]>> = None;

        while !input.is_empty() {
            let key: Ident = input.parse()?;
            if key == "Context" {
                input.parse::<Token![=]>()?;
                context_type = Some(input.parse()?);
            } else if key == "Event" {
                input.parse::<Token![=]>()?;
                event_type = Some(input.parse()?);
            } else if key == "State" {
                input.parse::<Token![=]>()?;
                state_type = Some(input.parse()?);
            } else if key == "initial" {
                input.parse::<Token![:]>()?;
                initial_state_variant = Some(input.parse()?); // Parse the state variant path (e.g., MyState::Initial)
                let content;
                braced!(content in input); // Parse the braced content `{ ... }`
                initial_context_fields = Some(content.parse_terminated(InitialContextField::parse, Token![,])?);
            } else {
                // ここで他のキー（例: "states"）のパースを追加する
                return Err(syn::Error::new(key.span(), format!("Unexpected keyword during initial parsing: {}", key)));
            }

            // Optional comma separator, allowing trailing comma
            if input.peek(Token![,]) {
                input.parse::<Token![,]>()?;
            } else if !input.is_empty() {
                 // If not a comma and not empty, it's an error unless it was the last item
                 // Handle cases where 'states:' might follow without a comma later
                 // return Err(input.error("Expected comma after field definition or end of input"));
            }
        }

        Ok(MachineDefinition {
            machine_name,
            context_type: context_type.ok_or_else(|| input.error("Missing 'Context = Type'"))?,
            event_type: event_type.ok_or_else(|| input.error("Missing 'Event = Type'"))?,
            state_type: state_type.ok_or_else(|| input.error("Missing 'State = Type'"))?,
            initial_state_variant: initial_state_variant.ok_or_else(|| input.error("Missing 'initial: StateVariant { ... }'"))?,
            initial_context_fields: initial_context_fields.ok_or_else(|| input.error("Missing initial context fields '{ ... }'"))?,
        })
    }
}

/// XState v5 ライクな定義から ActorLogic を実装する構造体を生成するマクロ。
#[proc_macro]
pub fn create_machine(input: TokenStream) -> TokenStream {
    // 入力を MachineDefinition として解析
    let def = parse_macro_input!(input as MachineDefinition);

    // 解析結果から情報を抽出
    let machine_name = def.machine_name;
    let context_type = def.context_type;
    let event_type = def.event_type;
    let state_type = def.state_type;
    let initial_state_variant = def.initial_state_variant;

    // initial context のフィールド名と値を取得
    let initial_ctx_members = def.initial_context_fields.iter().map(|f| &f.member);
    let initial_ctx_exprs = def.initial_context_fields.iter().map(|f| &f.expr);

    // コード生成
    let expanded = quote! {
        // 生成されるステートマシンロジック構造体
        #[derive(Debug, Clone, Default)]
        pub struct #machine_name;

        // ActorLogic トレイトの実装
        // rustate_core と async_trait へのパスを絶対パス (::) で指定
        #[::async_trait::async_trait]
        impl ::rustate_core::logic::ActorLogic for #machine_name {
            type Context = #context_type;
            type Event = #event_type;
            type State = #state_type;

            // initial メソッドの実装
            fn initial(&self) -> (Self::State, Self::Context) {
                (
                    #initial_state_variant, // 解析した初期状態バリアント
                    #context_type { // 解析したコンテキスト型
                        #( #initial_ctx_members: #initial_ctx_exprs ),* // 解析したフィールドを初期化
                    }
                )
            }

            // transition メソッド (ダミー)
            async fn transition(
                &self,
                state: Self::State,
                context: Self::Context,
                event: Self::Event,
            ) -> Result<(Self::State, Self::Context), ::rustate_core::actor::ActorError> {
                println!(
                    "WARN: Transition logic not yet implemented for {}. State: {:?}, Context: {:?}, Event: {:?}",
                    stringify!(#machine_name), state, context, event
                );
                // TODO: ここでマクロ入力の遷移定義に基づいてロジックを生成する
                Ok((state, context)) // とりあえず現状維持
            }
        }
    };

    // 生成されたコードをトークンストリームとして返す
    TokenStream::from(expanded)
} 