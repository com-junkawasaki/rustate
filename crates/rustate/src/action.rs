use crate::Error;
use crate::Error::ActionError;
use crate::{Context, Event, EventTrait, Result};
use async_trait::async_trait;
use futures::future::BoxFuture;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::marker::PhantomData;
use serde::{de::{self, Visitor}, ser::{SerializeStruct}, Deserializer, Serializer};

/// Type alias for the action executor function
pub type ActionExecutor =
    Box<dyn Fn(&mut Context, &Event) -> Pin<Box<dyn Future<Output = ()> + Send>> + Send + Sync>;

/// Type of action execution
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActionType {
    /// Action executed when entering a state
    Entry,
    /// Action executed when exiting a state
    Exit,
    /// Action executed during a transition
    Transition,
}

/// Define Action as generic over C, E
#[derive(Clone)]
pub struct Action<C = Context, E = Event>
where
    C: Clone + Send + Sync + 'static,
    E: EventTrait + Send + Sync + 'static,
{
    /// The name of this action
    pub name: String,
    /// The type of action execution
    pub action_type: ActionType,
    /// Function pointer to execute the action (skipped during serialization/deserialization)
    #[allow(clippy::type_complexity)]
    pub(crate) execute_fn: Arc<dyn Fn(&mut C, &E) -> BoxFuture<'static, ()> + Send + Sync>,
    _phantom_c: PhantomData<C>,
    _phantom_e: PhantomData<E>,
}

// Need to manually implement PartialEq because BoxFuture is not PartialEq
impl<C, E> PartialEq for Action<C, E>
where
    C: Clone + Send + Sync + 'static,
    E: EventTrait + Send + Sync + 'static,
{
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name && self.action_type == other.action_type
        // We cannot compare the execute_fn closures directly
    }
}

// If PartialEq is manually implemented, Eq can often be derived or implemented simply.
impl<C, E> Eq for Action<C, E>
where
    C: Clone + Send + Sync + 'static,
    E: EventTrait + Send + Sync + 'static,
{
}

// Need to manually implement Debug because BoxFuture is not Debug
impl<C, E> fmt::Debug for Action<C, E>
where
    C: Clone + Send + Sync + 'static,
    E: EventTrait + Send + Sync + 'static,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Action")
            .field("name", &self.name)
            .field("execute_fn", &"<Fn>") // Indicate function presence without printing
            .field("action_type", &self.action_type)
            .finish()
    }
}

impl<C, E> Action<C, E>
where
    C: Clone + Send + Sync + Default + 'static,
    E: EventTrait + Send + Sync + 'static,
{
    /// Create a new action with an async function
    pub fn new<F>(name: impl Into<String>, action_type: ActionType, execute_fn: F) -> Self
    where
        F: Fn(&mut C, &E) -> BoxFuture<'static, ()> + Send + Sync + 'static,
    {
        Self {
            name: name.into(),
            action_type,
            execute_fn: Arc::new(execute_fn),
            _phantom_c: PhantomData,
            _phantom_e: PhantomData,
        }
    }

    /// Create a new action with a synchronous function
    pub fn new_sync<F>(name: impl Into<String>, action_type: ActionType, execute_fn: F) -> Self
    where
        F: Fn(&mut C, &E) + Send + Sync + 'static,
    {
        let async_fn = move |ctx: &mut C, evt: &E| {
            execute_fn(ctx, evt);
            Box::pin(async {}) as BoxFuture<'static, ()>
        };
        Self {
            name: name.into(),
            action_type,
            execute_fn: Arc::new(async_fn),
            _phantom_c: PhantomData,
            _phantom_e: PhantomData,
        }
    }

    /// Execute the action.
    /// Note: The underlying execute_fn returns BoxFuture<'static, ()>, not a Result.
    /// Error handling should be implemented *within* the provided future if needed.
    pub async fn execute(&self, context: &mut C, event: &E) {
        // Directly await the future. If the future itself panics, the task will panic.
        (self.execute_fn)(context, event).await;
    }
}

impl<C, E> Action<C, E>
where
    C: Clone + Send + Sync + Default + 'static,
    E: EventTrait + Send + Sync + 'static,
{
    /// Create a new entry action
    pub fn entry<F>(name: impl Into<String>, execute_fn: F) -> Self
    where
        F: Fn(&mut C, &E) -> BoxFuture<'static, ()> + Send + Sync + 'static,
    {
        Self::new(name, ActionType::Entry, execute_fn)
    }

    /// Create a new exit action
    pub fn exit<F>(name: impl Into<String>, execute_fn: F) -> Self
    where
        F: Fn(&mut C, &E) -> BoxFuture<'static, ()> + Send + Sync + 'static,
    {
        Self::new(name, ActionType::Exit, execute_fn)
    }

    /// Create a new transition action
    pub fn transition<F>(name: impl Into<String>, execute_fn: F) -> Self
    where
        F: Fn(&mut C, &E) -> BoxFuture<'static, ()> + Send + Sync + 'static,
    {
        Self::new(name, ActionType::Transition, execute_fn)
    }

    /// Create a new action with a name only (for serialization)
    pub fn named(name: impl Into<String>, action_type: ActionType) -> Self {
        Self {
            name: name.into(),
            action_type,
            execute_fn: Arc::new(|_ctx, _evt| Box::pin(async {})),
            _phantom_c: PhantomData,
            _phantom_e: PhantomData,
        }
    }

    /// Clone without the executor
    pub fn without_executor(&self) -> Self {
        Self {
            name: self.name.clone(),
            action_type: self.action_type,
            execute_fn: Arc::new(|_ctx, _evt| Box::pin(async {})),
            _phantom_c: PhantomData,
            _phantom_e: PhantomData,
        }
    }
}

impl<C, E> fmt::Display for Action<C, E>
where
    C: Clone + Send + Sync + Default + 'static,
    E: EventTrait + Send + Sync + 'static,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Action({}, {:?})", self.name, self.action_type)
    }
}

/// Trait for types that can be converted into an action.
pub trait IntoAction<C, E>
where
    C: Clone + Send + Sync + Default + 'static,
    E: EventTrait + Send + Sync + 'static,
{
    /// Converts the type into an action.
    fn into_action(self) -> Action<C, E>;
}

// Implement IntoAction for closures
impl<F, Fut, C, E> IntoAction<C, E> for F
where
    F: Fn(&mut C, &E) -> Fut + Send + Sync + 'static + Clone,
    Fut: Future<Output = ()> + Send + 'static, // Match execute_fn signature: Output is ()
    C: Clone + Send + Sync + Default + 'static,
    E: EventTrait + Send + Sync + 'static,
{
    fn into_action(self) -> Action<C, E> {
        // The closure F returns Fut which returns (), matching execute_fn's signature
        Action::new(
            "closure_action", // TODO: Allow naming?
            ActionType::Transition, // Default, consider allowing specification
            move |ctx, evt| Box::pin(self(ctx, evt)) // Box the future directly
        )
    }
}

// Implement IntoAction for Action itself
impl<C, E> IntoAction<C, E> for Action<C, E>
where
    C: Clone + Send + Sync + Default + 'static,
    E: EventTrait + Send + Sync + 'static,
{
    fn into_action(self) -> Action<C, E> {
        self
    }
}

// Manual Serialize implementation
impl<C, E> Serialize for Action<C, E>
where
    C: Clone + Send + Sync + 'static,
    E: EventTrait + Send + Sync + 'static,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("Action", 2)?;
        state.serialize_field("name", &self.name)?;
        state.serialize_field("action_type", &self.action_type)?;
        // execute_fn is skipped
        state.end()
    }
}

// Manual Deserialize implementation
impl<'de, C, E> Deserialize<'de> for Action<C, E>
where
    C: Clone + Send + Sync + Default + 'static, // Need Default for dummy fn
    E: EventTrait + Send + Sync + 'static,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        enum Field { Name, ActionType }

        impl<'de> Deserialize<'de> for Field {
            fn deserialize<D>(deserializer: D) -> Result<Field, D::Error>
            where
                D: Deserializer<'de>,
            {
                struct FieldVisitor;

                impl<'de> Visitor<'de> for FieldVisitor {
                    type Value = Field;

                    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                        formatter.write_str("`name` or `action_type`")
                    }

                    fn visit_str<E_>(self, value: &str) -> Result<Field, E_>
                    where
                        E_: de::Error,
                    {
                        match value {
                            "name" => Ok(Field::Name),
                            "action_type" => Ok(Field::ActionType),
                            _ => Err(de::Error::unknown_field(value, FIELDS)),
                        }
                    }
                }

                deserializer.deserialize_identifier(FieldVisitor)
            }
        }

        struct ActionVisitor<C, E> {
            _phantom_c: PhantomData<C>,
            _phantom_e: PhantomData<E>,
        }

        impl<'de, C, E> Visitor<'de> for ActionVisitor<C, E>
        where
            C: Clone + Send + Sync + Default + 'static,
            E: EventTrait + Send + Sync + 'static,
        {
            type Value = Action<C, E>;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("struct Action")
            }

            fn visit_map<V>(self, mut map: V) -> Result<Action<C, E>, V::Error>
            where
                V: de::MapAccess<'de>,
            {
                let mut name = None;
                let mut action_type = None;
                while let Some(key) = map.next_key()? {
                    match key {
                        Field::Name => {
                            if name.is_some() {
                                return Err(de::Error::duplicate_field("name"));
                            }
                            name = Some(map.next_value()?);
                        }
                        Field::ActionType => {
                            if action_type.is_some() {
                                return Err(de::Error::duplicate_field("action_type"));
                            }
                            action_type = Some(map.next_value()?);
                        }
                    }
                }
                let name = name.ok_or_else(|| de::Error::missing_field("name"))?;
                let action_type = action_type.ok_or_else(|| de::Error::missing_field("action_type"))?;
                Ok(Action {
                    name,
                    action_type,
                    // Provide a dummy function for deserialized actions
                    execute_fn: Arc::new(|_ctx, _evt| Box::pin(async {})),
                    _phantom_c: PhantomData,
                    _phantom_e: PhantomData,
                })
            }
        }

        const FIELDS: &'static [&'static str] = &["name", "action_type"];
        deserializer.deserialize_struct("Action", FIELDS, ActionVisitor { _phantom_c: PhantomData, _phantom_e: PhantomData })
    }
}
