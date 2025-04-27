use crate::{Context, Event, Machine, StateError};
use serde_json::Value;
use std::collections::HashMap;
use std::fmt;
use std::future::Future;
use std::pin::Pin;
use crate::EventTrait;

// ... existing code ... 