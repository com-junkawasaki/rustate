extern crate proc_macro;

use proc_macro::TokenStream;
use quote::quote;
use syn::parse::{Parse, ParseStream, Result};
use syn::punctuated::Punctuated;
use syn::{braced, parse_macro_input, Error, Expr, Ident, LitStr, Token, Type};

/// Represents a `FieldName: FieldValue` pair used for defining the initial context.
///
/// This struct is used during parsing to capture the fields and their initial values
/// provided within the `initial: { ... }` block of the macro input.
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

/// Represents a single transition definition within a state: `EVENT: "TargetState"`.
///
/// This struct captures the event identifier that triggers the transition and the
/// string literal representing the target state's name.
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

/// Represents the definition of a single state, including its name and transitions.
///
/// Parses a block like `StateName { on: { EVENT1: "Target1", EVENT2: "Target2" } }`.
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
                return Err(Error::new(
                    on_kw.span(),
                    "Expected 'on' keyword for transitions",
                ));
            }
        }

        Ok(StateDefinition {
            state_name,
            transitions,
        })
    }
}

/// Represents the overall structure of the `create_machine` macro input.
///
/// This struct holds all the parsed components of the machine definition,
/// including its name, associated types (Context, Event, State), initial state,
/// initial context, and state definitions.
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
                initial_context_fields =
                    Some(content.parse_terminated(InitialContextField::parse, Token![,])?);
            } else if key == "states" {
                input.parse::<Token![:]>()?;
                let content;
                braced!(content in input);
                states = Some(content.parse_terminated(StateDefinition::parse, Token![,])?);
            } else {
                return Err(syn::Error::new(
                    key.span(),
                    format!("Unexpected keyword during parsing: {}", key),
                ));
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
            initial_state_value: initial_state_value
                .ok_or_else(|| input.error("Missing 'initial: StateVariant { ... }'"))?,
            initial_context_fields: initial_context_fields
                .ok_or_else(|| input.error("Missing initial context fields '{ ... }'"))?,
            states: states.ok_or_else(|| input.error("Missing 'states: { ... }' block"))?,
        })
    }
}

/// Procedural macro to generate a state machine structure implementing `ActorLogic`.
///
/// This macro takes a definition resembling XState v5's machine configuration and
/// generates a Rust struct that implements the `rustate_core::logic::ActorLogic` trait.
///
/// # Usage
///
/// ```rust,ignore
/// use rustate_core::logic::ActorLogic;
/// use rustate_macros::create_machine;
/// use async_trait::async_trait;
///
/// #[derive(Debug, Clone, PartialEq)]
/// pub enum LightState {
///     Green,
///     Yellow,
///     Red,
/// }
///
/// #[derive(Debug, Clone)]
/// pub struct LightContext {
///     timer: u32,
/// }
///
/// #[derive(Debug, Clone)]
/// pub enum LightEvent {
///     TimerElapsed,
///     PowerOutage,
/// }
///
/// create_machine!(
///     TrafficLightMachine, // Name of the generated struct
///     Context: LightContext,
///     Event: LightEvent,
///     State: LightState,
///     initial: LightState::Red { // Initial state and context
///         timer: 0
///     },
///     states: { // State definitions
///         Red {
///             on: { // Transitions for the Red state
///                 TimerElapsed: "Green"
///             }
///         },
///         Yellow {
///             on: {
///                 TimerElapsed: "Red"
///             }
///         },
///         Green {
///             on: {
///                 TimerElapsed: "Yellow"
///             }
///         }
///     }
/// );
///
/// // The macro generates:
/// // #[derive(Debug, Clone, Default)]
/// // pub struct TrafficLightMachine;
/// //
/// // #[async_trait]
/// // impl ActorLogic for TrafficLightMachine {
/// //     type Context = LightContext;
/// //     type Event = LightEvent;
/// //     type State = LightState;
/// //
/// //     fn initial(&self) -> (Self::State, Self::Context) {
/// //         (LightState::Red, LightContext { timer: 0 })
/// //     }
/// //
/// //     async fn transition(
/// //         &self,
/// //         state: Self::State,
/// //         context: Self::Context,
/// //         event: Self::Event,
/// //     ) -> Result<(Self::State, Self::Context), ActorError> {
/// //         match (state.clone(), event) {
/// //             (LightState::Red, LightEvent::TimerElapsed { .. }) => Ok((LightState::Green, context)),
/// //             (LightState::Red, _) => Ok((state, context)), // No transition for other events
/// //             (LightState::Yellow, LightEvent::TimerElapsed { .. }) => Ok((LightState::Red, context)),
/// //             (LightState::Yellow, _) => Ok((state, context)),
/// //             (LightState::Green, LightEvent::TimerElapsed { .. }) => Ok((LightState::Yellow, context)),
/// //             (LightState::Green, _) => Ok((state, context)),
/// //         }
/// //     }
/// // }
/// ```
///
/// # Arguments
///
/// The macro input follows this structure:
///
/// 1.  `MachineName`: The identifier for the generated state machine struct.
/// 2.  `Context: Type`: The type used for the machine's context data.
/// 3.  `Event: Type`: The enum type representing events the machine can process.
/// 4.  `State: Type`: The enum type representing the possible states of the machine.
/// 5.  `initial: StateVariant { field1: value1, ... }`: Specifies the initial state variant and the initial values for the context fields.
/// 6.  `states: { StateName { on: { EVENT: "TargetState", ... } }, ... }`: Defines each state and its transitions.
///     - `StateName`: An identifier matching a variant in the `State` enum.
///     - `on: { ... }`: Contains the transition definitions for this state.
///         - `EVENT: "TargetState"`: Maps an `Event` enum variant to the name (as a string literal) of the target `State` variant.
///
/// # Generated Code
///
/// The macro generates:
/// - A unit struct named `MachineName` with `Debug`, `Clone`, and `Default` derives.
/// - An `async_trait` implementation of `ActorLogic` for the `MachineName` struct:
///     - Associated types `Context`, `Event`, and `State` are set to the provided types.
///     - The `initial` method returns the specified initial state and context.
///     - The `transition` method implements the state transitions based on the `states` definition. It matches on the current state and incoming event, returning the new state and unmodified context. If no transition is defined for a given event in the current state, it returns the current state and context.
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
        /// Generated state machine logic struct.
        /// Implements `rustate_core::logic::ActorLogic`.
        #[derive(Debug, Clone, Default)]
        pub struct #machine_name;

        #[::async_trait::async_trait]
        impl ::rustate_core::logic::ActorLogic for #machine_name {
            type Context = #context_type;
            type Event = #event_type;
            type State = #state_type;

            /// Returns the initial state and context for the machine.
            fn initial(&self) -> (Self::State, Self::Context) {
                (
                    #initial_state_value,
                    #context_type {
                        #( #initial_ctx_members: #initial_ctx_exprs ),*
                    }
                )
            }

            /// Processes an event and transitions the machine to a new state if applicable.
            ///
            /// # Arguments
            ///
            /// * `state` - The current state of the machine.
            /// * `context` - The current context of the machine.
            /// * `event` - The event to process.
            ///
            /// # Returns
            ///
            /// A `Result` containing the new state and context, or an `ActorError`.
            /// Currently, context is passed through unchanged.
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
