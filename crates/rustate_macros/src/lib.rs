extern crate proc_macro;

use proc_macro::TokenStream;
use quote::quote;
use syn::parse::{Parse, ParseStream, Result};
use syn::punctuated::Punctuated;
use syn::{
    braced, parenthesized, parse_macro_input, token, Expr, Ident, LitStr, Path, Token, Type, Lit, Error
};
use std::collections::HashMap;

// Represents `FieldName: FieldValue` used in initial context
#[derive(Clone)]
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

// Represents a single transition: EVENT: "TargetState" or EVENT: { target: "TargetState", actions: [...] }
#[derive(Clone)]
struct Transition {
    event_ident: Ident,
    target_state: LitStr,
}

impl Parse for Transition {
    fn parse(input: ParseStream) -> Result<Self> {
        let event_ident: Ident = input.parse()?;
        input.parse::<Token![:]>()?;
        let target_state: LitStr = input.parse()?;
        Ok(Transition {
            event_ident,
            target_state,
        })
    }
}

// Represents the definition of a single state, including its transitions
#[derive(Clone)]
struct StateDefinition {
    state_name: Ident,
    transitions: Punctuated<Transition, Token![,]>,
}

impl Parse for StateDefinition {
    fn parse(input: ParseStream) -> Result<Self> {
        let state_name: Ident = input.parse()?;
        let content;
        braced!(content in input);

        let mut transitions: Punctuated<Transition, Token![,]> = Punctuated::new();

        if !content.is_empty() && content.peek(Ident) && content.peek2(Token![:]) {
            let on_kw: Ident = content.parse()?;
            if on_kw == "on" {
                content.parse::<Token![:]>()?;
                let transitions_content;
                braced!(transitions_content in content);
                transitions = transitions_content.parse_terminated(Transition::parse, Token![,])?;
            } else {
                 return Err(Error::new(on_kw.span(), "Expected 'on' keyword for transitions"));
            }
        }

        Ok(StateDefinition {
            state_name,
            transitions,
        })
    }
}

// Represents the overall machine definition input
struct MachineDefinition {
    machine_name: Ident,
    context_type: Type,
    event_type: Type,
    state_type: Type,
    initial_state_value: Expr,
    initial_context_fields: Punctuated<InitialContextField, Token![,]>,
    states: Punctuated<StateDefinition, Token![,]>,
}

impl Parse for MachineDefinition {
    fn parse(input: ParseStream) -> Result<Self> {
        let machine_name: Ident = input.parse()?;
        input.parse::<Token![,]>()?;

        let mut context_type: Option<Type> = None;
        let mut event_type: Option<Type> = None;
        let mut state_type: Option<Type> = None;
        let mut initial_state_value: Option<Expr> = None;
        let mut initial_context_fields: Option<Punctuated<InitialContextField, Token![,]>> = None;
        let mut states: Option<Punctuated<StateDefinition, Token![,]>> = None;

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
                initial_state_value = Some(input.parse()?);
                let content;
                braced!(content in input);
                initial_context_fields = Some(content.parse_terminated(InitialContextField::parse, Token![,])?);
            } else if key == "states" {
                 input.parse::<Token![:]>()?;
                 let content;
                 braced!(content in input);
                 states = Some(content.parse_terminated(StateDefinition::parse, Token![,])?);
            } else {
                return Err(syn::Error::new(key.span(), format!("Unexpected keyword during parsing: {}", key)));
            }

            if input.peek(Token![,]) {
                input.parse::<Token![,]>()?;
            } else if !input.is_empty() {
                
            }
        }

        Ok(MachineDefinition {
            machine_name,
            context_type: context_type.ok_or_else(|| input.error("Missing 'Context = Type'"))?,
            event_type: event_type.ok_or_else(|| input.error("Missing 'Event = Type'"))?,
            state_type: state_type.ok_or_else(|| input.error("Missing 'State = Type'"))?,
            initial_state_value: initial_state_value.ok_or_else(|| input.error("Missing 'initial: StateVariant { ... }'"))?,
            initial_context_fields: initial_context_fields.ok_or_else(|| input.error("Missing initial context fields '{ ... }'"))?,
            states: states.ok_or_else(|| input.error("Missing 'states: { ... }' block"))?,
        })
    }
}

/// XState v5 ライクな定義から ActorLogic を実装する構造体を生成するマクロ。
#[proc_macro]
pub fn create_machine(input: TokenStream) -> TokenStream {
    let def = parse_macro_input!(input as MachineDefinition);

    let machine_name = &def.machine_name;
    let context_type = &def.context_type;
    let event_type = &def.event_type;
    let state_type = &def.state_type;
    let initial_state_value = &def.initial_state_value;

    let initial_ctx_members = def.initial_context_fields.iter().map(|f| &f.member);
    let initial_ctx_exprs = def.initial_context_fields.iter().map(|f| &f.expr);

    let mut transition_arms = Vec::new();
    let mut handled_states = std::collections::HashSet::new();

    for state_def in def.states.iter() {
        let current_state_name = &state_def.state_name;
        let current_state_path = quote! { #state_type::#current_state_name };
        handled_states.insert(current_state_name.to_string());

        for transition in state_def.transitions.iter() {
            let event_variant = &transition.event_ident;
            let target_state_str = &transition.target_state;
            let event_path = quote! { #event_type::#event_variant { .. } };
            let target_state_ident = Ident::new(&target_state_str.value(), target_state_str.span());
            let target_state_path = quote! { #state_type::#target_state_ident };

            transition_arms.push(quote! {
                (#current_state_path, #event_path) => {
                    println!("Transitioning from {} to {} on event {}",
                             stringify!(#current_state_name),
                             #target_state_str,
                             stringify!(#event_variant));
                    Ok((#target_state_path, context))
                }
            });
        }

        transition_arms.push(quote! {
            (#current_state_path, _) => {
                Ok((state, context))
            }
        });
    }

    let expanded = quote! {
        #[derive(Debug, Clone, Default)]
        pub struct #machine_name;

        #[::async_trait::async_trait]
        impl ::rustate_core::logic::ActorLogic for #machine_name {
            type Context = #context_type;
            type Event = #event_type;
            type State = #state_type;

            fn initial(&self) -> (Self::State, Self::Context) {
                (
                    #initial_state_value,
                    #context_type {
                        #( #initial_ctx_members: #initial_ctx_exprs ),*
                    }
                )
            }

            async fn transition(
                &self,
                state: Self::State,
                context: Self::Context,
                event: Self::Event,
            ) -> Result<(Self::State, Self::Context), ::rustate_core::actor::ActorError> {
                let current_state_for_match = state.clone();
                match (current_state_for_match, event) {
                    #( #transition_arms ),*
                }
            }
        }
    };

    TokenStream::from(expanded)
} 