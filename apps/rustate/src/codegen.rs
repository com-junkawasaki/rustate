//! Code generation from state machine definitions.
//!
//! This module provides functionality to export `Machine` definitions into
//! other formats like JSON or Protocol Buffers schemas.
//!
//! Requires the `codegen` feature flag. Protocol Buffers export specifically
//! requires the `proto` feature flag in addition to `codegen`.

use crate::{
    action::{Action, ActionType, Guard},
    context::{Context, ContextTrait},
    error::{Error, Result},
    event::{Event, EventTrait, IntoEvent},
    machine::MachineBuilder,
    state::{State, StateId, StateTrait},
    state_registry::StateRegistry,
    transition::Transition,
    Machine,
};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::Write;
use std::fmt::{Debug, Display};
use std::hash::Hash;
use serde::de::DeserializeOwned;
use syn::parse_file;

// Conditionally import items needed for specific features
#[cfg(feature = "codegen")]
use {};

// #[cfg(feature = "proto")] // Removed unused import
// use std::collections::HashMap;

/// Options for exporting a state machine definition to JSON format.
#[derive(Debug, Clone)]
pub struct JsonExportOptions {
    /// The path to the output JSON file.
    pub output_path: String,
    /// Whether to format the output JSON prettily (with indentation).
    pub pretty: bool,
    /// Whether to include internal metadata in the export (currently unused).
    pub include_metadata: bool, // Note: Machine serialization might need adjustment to use this
}

impl Default for JsonExportOptions {
    fn default() -> Self {
        Self {
            output_path: "statemachine.json".to_string(),
            pretty: true,
            include_metadata: false, // Defaulting to false as it's currently unused
        }
    }
}

/// Options for exporting a state machine definition to Protocol Buffers format.
#[derive(Debug, Clone)]
pub struct ProtoExportOptions {
    /// The path to the output `.proto` file.
    pub output_path: String,
    /// The package name declared in the `.proto` file.
    pub package_name: String,
    /// The name for the main `StateMachine` message in the `.proto` file.
    pub message_name: String,
}

impl Default for ProtoExportOptions {
    fn default() -> Self {
        Self {
            output_path: "statemachine.proto".to_string(),
            package_name: "rustate".to_string(),
            message_name: "StateMachine".to_string(),
        }
    }
}

/// Extension trait providing code generation capabilities for `Machine`.
/// Requires the `codegen` feature.
pub trait CodegenExt {
    /// Exports the state machine definition to a JSON file.
    ///
    /// # Arguments
    /// * `options` - Optional configuration for JSON export (path, formatting).
    ///             Defaults are used if `None`.
    ///
    /// Requires the `codegen` feature.
    fn export_to_json(&self, options: Option<JsonExportOptions>) -> Result<()>;

    /// Exports the state machine definition to a `.proto` file.
    ///
    /// **Note:** This currently generates a fixed, basic Protobuf schema.
    /// It does not dynamically adapt the schema based on the specific machine's
    /// states, events, or context structure.
    ///
    /// # Arguments
    /// * `options` - Optional configuration for Protobuf export (path, package name).
    ///             Defaults are used if `None`.
    ///
    /// Requires both the `codegen` and `proto` features.
    #[cfg(feature = "proto")]
    fn export_to_proto(&self, options: Option<ProtoExportOptions>) -> Result<()>;

    /// **(Placeholder)** Parses a state machine definition from a Rust source file.
    ///
    /// **Note:** This function currently parses the Rust file using `syn` but
    /// returns a hardcoded, simple machine definition. It does **not** yet
    /// extract the actual machine definition from the Abstract Syntax Tree (AST).
    ///
    /// # Arguments
    /// * `file_path` - Path to the Rust source file.
    ///
    /// Requires the `codegen` feature.
    #[cfg(feature = "codegen")]
    fn parse_from_rust_file(file_path: &str) -> Result<Self>
    where
        Self: Sized;
}

impl<C, E, S, O> CodegenExt for Machine<C, E, S, O>
where
    S: StateTrait + Serialize + Send + Sync + 'static,
    E: EventTrait + Serialize + Send + Sync + 'static,
    C: ContextTrait + Serialize + Send + Sync + 'static,
    O: Serialize + Send + Sync + 'static + Default,
{
    fn export_to_json(&self, options: Option<JsonExportOptions>) -> Result<()> {
        let options = options.unwrap_or_default();
        println!(
            "Exporting machine '{}' to JSON at: {}",
            self.name, options.output_path
        );

        // TODO: Factor in options.include_metadata if needed by modifying
        // the Machine's Serialize implementation or creating a specific export struct.

        let json_str = if options.pretty {
            serde_json::to_string_pretty(self)?
        } else {
            serde_json::to_string(self)?
        };

        let mut file = File::create(&options.output_path).map_err(|e| {
            Error::IoError(format!(
                "Failed to create JSON output file '{}': {}",
                options.output_path, e
            ))
        })?;

        file.write_all(json_str.as_bytes()).map_err(|e| {
            Error::IoError(format!(
                "Failed to write to JSON output file '{}': {}",
                options.output_path, e
            ))
        })?;
        println!("JSON export successful.");
        Ok(())
    }

    #[cfg(feature = "proto")]
    fn export_to_proto(&self, options: Option<ProtoExportOptions>) -> Result<()> {
        let options = options.unwrap_or_default();
        println!(
            "Exporting machine '{}' to Protobuf schema at: {}",
            self.name, options.output_path
        );

        // Generate the Protocol Buffers schema definition string.
        // Note: This currently generates a static schema structure.
        let proto_schema = generate_proto_schema(&options)?;

        let mut file = File::create(&options.output_path).map_err(|e| {
            Error::IoError(format!(
                "Failed to create proto output file '{}': {}",
                options.output_path, e
            ))
        })?;

        file.write_all(proto_schema.as_bytes()).map_err(|e| {
            Error::IoError(format!(
                "Failed to write to proto output file '{}': {}",
                options.output_path, e
            ))
        })?;
        println!("Protobuf schema export successful.");
        Ok(())
    }

    #[cfg(feature = "codegen")]
    fn parse_from_rust_file(file_path: &str) -> Result<Self> {
        println!(
            "Parsing Rust file for machine definition (Placeholder): {}",
            file_path
        );
        let source_code = std::fs::read_to_string(file_path).map_err(|e| {
            Error::IoError(format!("Failed to read source file '{}': {}", file_path, e))
        })?;

        let _syntax_tree = parse_file(&source_code).map_err(|e| {
            Error::InvalidConfiguration(format!("Failed to parse Rust file '{}': {}", file_path, e))
        })?;

        // --- Placeholder Implementation ---
        // This section needs to traverse the AST (_syntax_tree) and extract the
        // state machine definition (e.g., from MachineBuilder calls).
        eprintln!("Warning: parse_from_rust_file currently returns a hardcoded machine.");

        // Returning a simple hardcoded machine for now.
        // Needs concrete types or appropriate generics. Let's use basic ones for placeholder.
        // IMPORTANT: This placeholder likely doesn't match the expected Self type (Machine<C, E, S, O>).
        // This function needs significant refinement to be useful.
        // For now, let's assume C=(), E=String, S=String, O=() to satisfy the placeholder build.
        // This WILL likely fail if called unless Self is exactly Machine<(), String, String, ()>.
        let builder = crate::MachineBuilder::<(), String, String, ()>::new("placeholder_machine".to_string())
            .state("idle".to_string(), |s| {
                s.on("START".to_string(), |t| t.target("active".to_string()))
            })
            .state("active".to_string(), |s| {
                s.on("STOP".to_string(), |t| t.target("idle".to_string()))
            })
            .initial("idle".to_string()); // Explicitly set initial state

        // The build result must match Self. This is problematic for a generic impl.
        // This placeholder is fundamentally flawed for a generic CodegenExt impl.
        // It should likely be a standalone function or specific to a concrete Machine type.
        // Forcing a build that *might* conform to Self (if Self is the specific placeholder type)
        let _machine = builder.build()?; // Assign to _machine to avoid unused variable warning
        // This is unsafe and likely incorrect. A real implementation needs AST traversal.
        // We'll return an error or a more robust placeholder.
        // Let's return an error indicating it's unimplemented.
        Err(Error::InvalidConfiguration("parse_from_rust_file is not fully implemented".to_string()))

        // If we absolutely needed to return *something* matching Self (very risky):
        // Ok(unsafe { std::mem::transmute_copy(&machine) }) // Extremely unsafe and wrong, DO NOT USE
    }
}

/// Generates a basic, static Protocol Buffers schema string.
#[cfg(feature = "proto")]
fn generate_proto_schema(options: &ProtoExportOptions) -> Result<String> {
    // This generates a fixed schema structure. It doesn't inspect the actual Machine.
    // Future improvements could involve reflecting on the Machine structure
    // (if possible and practical) or taking a more detailed definition format as input.
    let schema = format!(
        r#"
// Generated by RuState codegen (basic schema)

syntax = "proto3";

package {package_name};

// Basic representation of a state
message State {{
  string id = 1; // Unique identifier for the state (e.g., "idle", "active.loading")
  // Add other fields as needed, e.g., type (atomic, compound, parallel), parent
}}

// Basic representation of a transition
message Transition {{
  string source_state_id = 1;
  string event_name = 2;
  string target_state_id = 3;
  // Add other fields like guard conditions, actions
}}

// Basic representation of an action (placeholder)
message Action {{
  string id = 1; // Identifier for the action
  // Add details about the action type and parameters
}}

// Main message representing the state machine structure
message {message_name} {{
  string name = 1;            // Identifier for the machine
  string initial_state_id = 2; // ID of the initial state
  map<string, State> states = 3; // Map of state ID to State message
  repeated Transition transitions = 4; // List of all transitions
  repeated Action actions = 5;      // List of actions (e.g., entry/exit)
  // Add context definition if possible/needed
}}
"#,
        package_name = options.package_name,
        message_name = options.message_name
    );

    Ok(schema)
}

/// Helper function to serialize a `MachineBuilder` directly to a JSON string.
/// Builds the machine internally before serializing.
/// Requires the `codegen` feature.
#[cfg(feature = "codegen")]
pub fn machine_builder_to_json<C, E, S, O>(
    builder: &mut crate::MachineBuilder<C, E, S, O>,
) -> Result<String>
where
    S: StateTrait + Send + Sync + Clone + 'static + Serialize,
    E: EventTrait + Send + Sync + Clone + 'static + Serialize,
    C: ContextTrait + Send + Sync + Clone + 'static + Serialize,
    O: Default + Send + Sync + Clone + 'static + Serialize,
{
    // Placeholder: serialize the builder directly if possible, or extract data
    println!("Warning: machine_builder_to_json is a placeholder.");
    // Attempt to serialize the builder (might not work depending on Serialize impl)
     serde_json::to_string_pretty(builder).map_err(Error::from)
    // Or extract relevant data and serialize that
    // Example:
    // let data = BuilderExportData { name: builder.name.clone(), ... };
    // serde_json::to_string_pretty(&data).map_err(Error::from)
}

/// Helper function to generate a basic Protocol Buffers schema from a `MachineBuilder`.
/// **Note:** Builds the machine internally but currently ignores its structure
/// and generates a static schema using `generate_proto_schema`.
/// Requires the `codegen` and `proto` features.
#[cfg(all(feature = "codegen", feature = "proto"))]
pub fn machine_builder_to_proto<
    S: crate::StateTrait + Send + Sync + Clone + 'static + Serialize,
    E: crate::EventTrait + Send + Sync + Clone + 'static + Serialize,
    C: crate::Context + Send + Sync + Clone + 'static + Serialize,
    O: crate::Context + Send + Sync + Clone + 'static + Serialize,
>(
    _builder: &mut crate::MachineBuilder<S, E, C, O>,
    options: Option<ProtoExportOptions>,
) -> Result<String> {
    let opts = options.unwrap_or_default();
    println!("Warning: machine_builder_to_proto is a placeholder.");
    generate_proto_schema(&opts)
}

#[cfg(feature = "codegen")]
fn generate_rust_actions<C, E>(
    actions: &[Action<C, E>],
    // Correct bounds for C and E needed if Action uses them in Debug/Display etc.
) -> Result<String>
where
    // S: StateTrait + Send + Sync + Clone + 'static + Serialize, // S not used here
    // E: EventTrait + Send + Sync + Clone + 'static + Serialize + Debug, // Add Debug if Action name uses it
    // C: ContextTrait + Clone + Debug + Default + Send + Sync + 'static + Serialize, // Add ContextTrait
    // Added bounds based on Action usage (assuming Debug for name)
    E: EventTrait + Debug,
    C: ContextTrait,
{
    let mut code = String::new();
    for action in actions {
        // Assuming action.name() gives a usable representation
        code.push_str(&format!("        // Action: {:?}\n", action.name()));
        // TODO: Generate actual Rust code for the action if possible/needed
        // This might involve serializing closures or function pointers, which is complex.
        // For now, just commenting the action name.
    }
    Ok(code)
}

#[cfg(feature = "codegen")]
fn generate_rust_guards<C, E>(
    guards: &[Guard<C, E>],
    // Correct bounds for C and E needed if Guard uses them
) -> Result<String>
where
    // S: StateTrait + Send + Sync + Clone + 'static + Serialize, // S not used
    // E: EventTrait + Send + Sync + Clone + 'static + Serialize + Debug, // Add Debug if Guard name uses it
    // C: ContextTrait + Clone + Debug + Default + Send + Sync + 'static + Serialize, // Add ContextTrait
    // Added bounds based on Guard usage (assuming Debug for name)
    E: EventTrait + Debug,
    C: ContextTrait + Debug,
{
    let mut code = String::new();
    for guard in guards {
         // Assuming guard.name gives a usable representation (needs Guard to have a name() method)
        code.push_str(&format!("        // Guard: {:?}\n", guard.name())); // Assuming guard has name()
        // TODO: Generate actual Rust code for the guard if possible/needed. Complex.
    }
    Ok(code)
}

#[cfg(feature = "codegen")]
fn generate_rust_transitions<S, E, C>(
    transitions: &[Transition<S, C, E>],
    // machine_name: &str, // machine_name not used here
    // Correct bounds for C, E, S
) -> Result<String>
where
    S: StateTrait + Debug, // Add Debug for source/target
    E: EventTrait + Debug, // Add Debug for event
    C: ContextTrait,
{
    let mut code = String::new();
    for transition in transitions {
        let target_code = match &transition.target {
            Some(target) => format!(".target({:?})", target),
            None => String::new(), // No target
        };
        let guard_code = if let Some(guard) = &transition.guard {
            // TODO: Generate guard code - complex
             format!(".guard(/* Guard: {:?} */)", guard.name()) // Assuming guard.name() exists
        } else {
            String::new()
        };
        let action_code = if let Some(action) = &transition.action {
             // TODO: Generate action code - complex
            format!(".action(/* Action: {:?} */)", action.name()) // Assuming action.name() exists
        } else {
            String::new()
        };

        code.push_str(&format!(
            "        .on({:?}, |t| t{}{}{})
", // Assuming event implements Debug
            transition.event, target_code, guard_code, action_code
        ));
    }
    Ok(code)
}

#[cfg(feature = "codegen")]
pub fn generate_rust_code<C, E, S, O>(
    builder: &MachineBuilder<C, E, S, O>,
    // Correct bounds for C, E, S, O
) -> Result<String>
where
    S: StateTrait + Debug + Clone + Eq + Hash, // Added Eq + Hash (used internally by builder likely)
    E: EventTrait + Debug + Clone + Eq + Hash, // Added Clone + Eq + Hash
    C: ContextTrait + Debug + Clone + Default, // Added Clone + Default
    O: Default + Clone + Debug, // Added Clone + Debug
{
    let machine_name = &builder.name;
    let mut code = format!(
        "use rustate::{{MachineBuilder, Action, Guard, Transition, State}};

"
    );
    // TODO: Add necessary use statements for Context, Event, State types if they are custom
    // Determine concrete type names (this is difficult generically)
    let context_type_name = std::any::type_name::<C>();
    let event_type_name = std::any::type_name::<E>();
    let state_type_name = std::any::type_name::<S>();
    let output_type_name = std::any::type_name::<O>();


    code.push_str(&format!(
        "fn build_{}_machine() -> rustate::Result<rustate::Machine<{}, {}, {}, {}>> {{
",
        machine_name.replace(|c: char| !c.is_alphanumeric(), "_"), // Sanitize name
        context_type_name, event_type_name, state_type_name, output_type_name
    ));
    code.push_str(&format!(
        "    let mut builder = MachineBuilder::<{}, {}, {}, {}>::new("{}".to_string());

",
        context_type_name, event_type_name, state_type_name, output_type_name,
        machine_name
    ));

    // Iterate through states and generate code
    for state_id in builder.get_state_ids() { // Assuming get_state_ids returns Vec<S>
        if let Some(state_def) = builder.get_state_definition(&state_id) { // Pass borrow
             // Use Debug formatting for state_id
            code.push_str(&format!("    builder = builder.state({:?}, |s| {{
", state_id));

            // Add transitions
            let transitions_code = generate_rust_transitions::<S, E, C>(&state_def.transitions)?; // Pass C explicitly
            code.push_str(&transitions_code);

            // Add entry/exit actions (assuming accessible & Action has name())
            // for action in &state_def.entry_actions {
            //     code.push_str(&format!("        // Entry Action: {:?}
", action.name()));
            // }
            // for action in &state_def.exit_actions {
            //     code.push_str(&format!("        // Exit Action: {:?}
", action.name()));
            // }


            // Handle nested states recursively? This needs access to nested state definitions.
            // if let Some(nested_machine_builder) = state_def.nested_machine_builder { ... }

            code.push_str("    });

");
        }
    }


    // Set initial state
    if let Some(initial_state) = &builder.initial_state { // Assuming initial_state is Option<S>
         // Use Debug formatting for initial_state
        code.push_str(&format!("    builder = builder.initial({:?});
", initial_state));
    }

    code.push_str("
    builder.build()
");
    code.push_str("}
");

    Ok(code)
}

#[cfg(feature = "proto")]
fn generate_grpc_code<S, E, C>(machine: &Machine<S, E, C>) -> Result<String> {
    // ... implementation ...
    Ok("".to_string())
}

/// Generates Rust code for the state machine definition.
pub fn generate_machine_code<
    S: StateTrait
        + Display
        + Clone
        + Eq
        + Hash
        + Send
        + Sync
        + 'static
        + From<String>
        + Serialize
        + DeserializeOwned
        + Debug,
    E: EventTrait
        + Clone
        + Eq
        + Hash
        + Send
        + Sync
        + 'static
        + Serialize
        + DeserializeOwned
        + Debug
        + IntoEvent
        + Display,
    C: ContextTrait + Send + Sync + Clone + 'static + Serialize + Default + Debug + DeserializeOwned,
    O: Default + Debug + Clone + Send + Sync + 'static + Serialize + DeserializeOwned,
>(
    // Specify generic parameters here
    machine_name: &str,
    builder: &mut crate::MachineBuilder<C, E, S, O>,
    initial_state: &S,
) -> Result<String> {
    // ... function body ...
    Ok(String::new()) // Placeholder return
}

// Placeholder for generating the machine struct definition string
// Corrected generic parameters C, E, S
pub fn generate_machine_struct_code<S, E, C>(
    machine_name: &str,
    // states: &[State<S, C, E>], // Need state definitions from builder
    // Need access to context type definition C
) -> Result<String>
where
    S: StateTrait, // Add necessary bounds if struct depends on them
    E: EventTrait,
    C: ContextTrait,
{
     println!("Warning: generate_machine_struct_code is a placeholder.");
     // Sanitize machine_name for struct identifier, replacing non-alphanumeric with hyphen
     let struct_name = machine_name.replace(|c: char| !c.is_alphanumeric(), '-');
     // Ensure the first char is valid if it was replaced (e.g., prepend "Machine")
     let struct_name = if struct_name.starts_with('-') { format!("Machine{}", struct_name) } else { struct_name };
     // Replace hyphens with underscores for valid identifier
     let struct_name = struct_name.replace('-', '_'); // Use char '_'

     // Corrected: Use \n for newline in the format string
     Ok(format!("struct {}Machine {{ /* ... fields ... */ }}\n", struct_name))
     // Corrected: Use hyphen in comment string to avoid prefix issue
     // Err(Error::InvalidConfiguration("Machine struct generation not-implemented".to_string()))
}

pub fn generate_state_enum_code<S: StateTrait + Display + Debug + Clone + Eq + Hash + Serialize + DeserializeOwned>(
    state_ids: &[S] // Or just IDs
) -> String {
    println!("Warning: generate_state_enum_code is a placeholder.");
    let variants = state_ids.iter().map(|s| format!("    {},", s)).collect::<Vec<_>>().join("
");
    // Use a sanitized name like StateEnum to avoid conflicts with crate::State
    format!("#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum StateEnum {{
{}
}}
", variants)
}

pub fn generate_event_enum_code<E: EventTrait + Display + Debug + Clone + Eq + Hash + Serialize + DeserializeOwned>(
    event_ids: &[E] // Or just IDs/names
) -> String {
     println!("Warning: generate_event_enum_code is a placeholder.");
    let variants = event_ids.iter().map(|e| format!("    {},", e)).collect::<Vec<_>>().join("
");
     // Use a sanitized name like EventEnum to avoid conflicts with crate::Event
    format!("#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EventEnum {{
{}
}}
", variants)
}

pub fn generate_context_struct_code<C: ContextTrait + Debug + Clone + Default + Serialize + DeserializeOwned>(
    context_type_name: &str // Name of the context type
) -> String {
    println!("Warning: generate_context_struct_code is a placeholder.");
    // Ensure context_type_name is a valid Rust identifier
    let struct_name = context_type_name.split("::").last().unwrap_or("Context"); // Basic sanitization
    format!("#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct {} {{ /* ... fields ... */ }}
", struct_name)
}

pub fn generate_action_enum_code<C, E>(
    action_names: &[String]
) -> String
where
    C: ContextTrait + Clone + Debug + Default + Send + Sync + 'static + Serialize + DeserializeOwned, // Added ContextTrait, DeserializeOwned
    E: EventTrait
        + Clone
        + Eq
        + Hash
        + Send
        + Sync
        + 'static
        + Serialize
        + DeserializeOwned
        + Debug,
{
     println!("Warning: generate_action_enum_code is a placeholder.");
    let variants = action_names.iter().map(|a| format!("    {},", a)).collect::<Vec<_>>().join("
");
     // Use a sanitized name like ActionEnum to avoid conflicts with crate::Action
    format!("#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ActionEnum {{
{}
}}
", variants)
}

// Placeholder for generating the builder setup code string
// Corrected generic parameter order C, E, S, O
pub fn generate_builder_code<
    S: StateTrait
        + Display
        + Clone
        + Eq
        + Hash
        + Send
        + Sync
        + 'static
        + From<String>
        + Serialize
        + DeserializeOwned
        + Debug, // Added Debug for initial state
    E: EventTrait
        + Clone
        + Eq
        + Hash
        + Send
        + Sync
        + 'static
        + Serialize
        + DeserializeOwned
        + Debug
        + IntoEvent
        + Display, // Added Display
    C: ContextTrait + Clone + Debug + Default + Send + Sync + 'static + Serialize + DeserializeOwned, // Added ContextTrait
    O: Clone + Debug + Default + Send + Sync + 'static + Serialize + DeserializeOwned,
>(
    machine_name: &str,
    // Need access to builder's internal state (states, transitions, initial)
    // states: &[State<S, C, E>],
    // transitions: &[Transition<S, C, E>],
    // initial_state_id: &S,
    builder: &MachineBuilder<C, E, S, O>, // Pass the builder itself
) -> Result<String> {
    println!("Warning: generate_builder_code is a placeholder.");
    generate_rust_code(builder) // Reuse the function generating builder code
    // Corrected: Use hyphen in comment string and ensure it's properly commented/terminated
    // Err(Error::InvalidConfiguration("Builder code generation not-implemented".to_string()))
}

impl<C, E, S, O> MachineBuilder<C, E, S, O>
where S: StateTrait + Eq + Hash + Clone, // Bounds for HashMap keys
      E: Eq + Hash + Clone, // Bounds for HashSet keys
      C: ContextTrait, // Added C bound needed for get_state_definition
      O: Default, // Added O bound needed for get_state_definition
{
    // NOTE: These methods assume access to internal fields `states` and potentially others.
    // They might need adjustment based on the actual MachineBuilder structure.
    // Consider making these public methods on MachineBuilder if they don't exist.

    // Assuming self.states is HashMap<S, StateDefinition<C, E, S, O>>
    fn get_state_ids(&self) -> Vec<S> {
         self.states.keys().cloned().collect()
    }

    fn get_event_ids(&self) -> Vec<E> {
        let mut events = HashSet::new();
        for state_def in self.states.values() {
            for transition in &state_def.transitions {
                 events.insert(transition.event.clone());
            }
            // Also consider events from entry/exit actions if applicable
        }
        events.into_iter().collect()
    }
     // Assuming get_state_definition exists and is public or accessible
     fn get_state_definition(&self, state_id: &S) -> Option<&crate::state::StateDefinition<C, E, S, O>> {
         self.states.get(state_id)
     }
}
