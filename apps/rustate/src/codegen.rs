//! Code generation from state machine definitions.
//!
//! This module provides functionality to export `Machine` definitions into
//! other formats like JSON or Protocol Buffers schemas.
//!
//! Requires the `codegen` feature flag. Protocol Buffers export specifically
//! requires the `proto` feature flag in addition to `codegen`.

use crate::{
    action::Action,
    event::{EventTrait, IntoEvent},
    machine::MachineBuilder,
    state::StateTrait,
    transition::Transition,
    Error, Guard, Machine, Result,
};
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::collections::HashSet;
use std::fmt::{Debug, Display};
use std::fs::File;
use std::hash::Hash;
use std::io::Write;
use syn::parse_file;

// Conditionally import items needed for specific features
// #[cfg(feature = \"codegen\")] // No specific imports needed here for now

// #[cfg(feature = \"proto\")] // Removed unused import
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
    S: StateTrait + Serialize + Send + Sync + 'static + From<String>,
    E: EventTrait + Serialize + Send + Sync + 'static + IntoEvent + Default + DeserializeOwned,
    C: Serialize + DeserializeOwned + Clone + Default + Send + Sync + Debug + 'static,
    O: Serialize + Send + Sync + 'static + Default + Clone + Debug + DeserializeOwned,
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

        // --- Placeholder Implementation - AST parsing needed ---
        println!("Warning: AST parsing not implemented in parse_from_rust_file.");
        todo!("Implement AST parsing to build MachineBuilder from Rust source");

        // Need to handle potential errors during build
        // The build result must match Self. This is problematic for a generic impl.
        // This placeholder likely doesn't match the expected Self type (Machine<C, E, S, O>).
        // This function needs significant refinement to be useful.
        // Forcing a build that *might* conform to Self (if Self is the specific placeholder type)
        // let _machine = builder.build()?; // Assign to _machine to avoid unused variable warning

        // Ok(Self {..}) // This needs to be constructed based on the parsed data
        // Return an error or the placeholder machine for now
        // Err(Error::NotImplemented("AST parsing for Machine creation".to_string()))
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
    S: StateTrait + Send + Sync + Clone + 'static + Serialize + From<String>,
    E: EventTrait
        + Send
        + Sync
        + Clone
        + 'static
        + Serialize
        + IntoEvent
        + Default
        + DeserializeOwned,
    C: Clone + Default + Send + Sync + Debug + 'static + Serialize + DeserializeOwned,
    O: Default + Send + Sync + Clone + 'static + Serialize + Debug + DeserializeOwned,
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
    C: Serialize + DeserializeOwned + Clone + Default + Send + Sync + Debug + 'static,
    O: Serialize + DeserializeOwned + Clone + Default + Send + Sync + Debug + 'static,
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
    E: EventTrait + Debug,
    C: Clone + Debug + Default + Send + Sync + 'static + Serialize + DeserializeOwned, // Removed ContextTrait
{
    let mut actions_code = String::new();
    for action in actions {
        // Assuming Action can be Debug printed or has a specific format
        // TODO: Need a proper way to represent actions in generated code (e.g., function calls?)
        actions_code.push_str(&format!("        // Action: {:?}\n", action)); // Temporary Debug print
    }
    Ok(actions_code)
}

#[cfg(feature = "codegen")]
fn generate_rust_guards<C, E>(
    guards: &[Guard<C, E>],
    // Correct bounds for C and E needed if Guard uses them
) -> Result<String>
where
    E: EventTrait + Debug,
    C: Clone + Debug + Default + Send + Sync + 'static + Serialize + DeserializeOwned + Debug, // Removed ContextTrait, added Debug
{
    let mut guards_code = String::new();
    for guard in guards {
        // Assuming guard has a name field or can be Debug printed
        guards_code.push_str(&format!("        // Guard: {:?}\n", guard)); // Assuming guard can be debug printed
    }
    Ok(guards_code)
}

#[cfg(feature = "codegen")]
fn generate_rust_transitions<S, E, C>(
    transitions: &[Transition<S, C, E>],
    // machine_name: &str, // machine_name not used here
    // Correct bounds for C, E, S
) -> Result<String>
where
    S: StateTrait + Debug, // Add Debug for source/target
    E: EventTrait + Debug + Serialize + DeserializeOwned, // Removed Deref bound
    C: Clone + Debug + Default + Send + Sync + 'static + Serialize + DeserializeOwned,
{
    let mut transitions_code = String::new();
    for transition in transitions {
        // Use Debug representation for the optional event
        let event_str = format!("{:?}", transition.event);
        let target_str = transition
            .target
            .as_ref()
            .map_or("None".to_string(), |t| format!("Some({})", t));
        let guard_str = if let Some(guard) = &transition.guard {
            format!(".guard({})", guard.name) // Use field access
        } else {
            "".to_string()
        };
        let action_code = if !transition.actions.is_empty() {
            // TODO: How to represent multiple actions? Chain calls?
            // format!(".action({})", action.name()) // Placeholder
            format!(".actions({:?})", transition.actions) // Temporary Debug
        } else {
            "".to_string()
        };

        transitions_code.push_str(&format!(
            "        .on({}, |t| t{}{}{})\n", // Use {} for event_str (now a String)
            event_str, target_str, guard_str, action_code
        ));
    }
    Ok(transitions_code)
}

#[cfg(feature = "codegen")]
pub fn generate_rust_code<C, E, S, O>(
    builder: &MachineBuilder<C, E, S, O>,
    // Correct bounds for C, E, S, O
) -> Result<String>
where
    S: StateTrait + Display + Debug + Clone + Eq + Hash + From<String>, // Added Display for state_enum_code
    E: EventTrait
        + Display
        + Debug
        + Clone
        + Eq
        + Hash
        + Serialize
        + IntoEvent
        + Default
        + DeserializeOwned, // Added Display
    C: Clone + Debug + Default + Send + Sync + 'static + Serialize + DeserializeOwned, // Removed ContextTrait
    O: Default + Clone + Debug + Serialize + Send + Sync + DeserializeOwned, // Added Serialize, Send, Sync, DeserializeOwned
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
        context_type_name,
        event_type_name,
        state_type_name,
        output_type_name
    ));
    code.push_str(&format!(
        "    let mut builder = MachineBuilder::<{}, {}, {}, {}>::new({:?}.to_string());\n\n", // Corrected: Use machine_name
        context_type_name, event_type_name, state_type_name, output_type_name, machine_name
    ));

    // Iterate through states and generate code
    let state_ids = builder.get_state_ids();
    let event_ids = builder.get_event_ids(); // Assuming E implements needed traits (Debug, Clone, Eq, Hash)

    let state_enum_name = format!("{}State", machine_name);
    let event_enum_name = format!("{}Event", machine_name);
    let context_type_name = std::any::type_name::<C>()
        .split("::")
        .last()
        .unwrap_or("Context");

    let state_enum = generate_state_enum_code(&state_ids);
    let event_enum = generate_event_enum_code(&event_ids);

    // Generate context struct based on the type C
    let context_struct = generate_context_struct_code::<C>(context_type_name);

    // Add initial state setting
    // builder.initial is guaranteed to exist by MachineBuilder::new
    let initial = &builder.initial;
    let initial_state_code = format!("{}.into()", initial);

    let builder_init = format!(
        "let mut builder = MachineBuilder::<{}, {}, {}, {}>::new(\"{}\", {});\n",
        std::any::type_name::<C>(), // C - Use actual type name
        event_enum_name,            // E - Use generated enum name
        state_enum_name,            // S - Use generated enum name
        std::any::type_name::<O>(), // O - Use actual type name
        machine_name,
        initial_state_code // Use the generated initial state code
    ); // Close format! macro

    // Combine generated parts
    let mut machine_code = String::new();
    machine_code.push_str(&builder_init);

    // Add state configurations to builder
    for state_id in builder.get_state_ids() {
        if let Some(state_def) = builder.get_state(&state_id) {
            let mut state_config =
                format!("    builder = builder.state({}.into(), |s| {{\n", state_id);

            // Add transitions for this state
            if !state_def.on.is_empty() {
                // Check if the `on` map itself is empty
                let transitions_code = generate_rust_transitions(
                    &state_def.on.values().flatten().cloned().collect::<Vec<_>>(),
                )?;
                if !transitions_code.trim().is_empty() {
                    // Only add if transitions were generated
                    state_config.push_str(&transitions_code);
                }
            }

            // Add entry/exit actions
            if !state_def.entry.is_empty() {
                let entry_actions_code = generate_rust_actions(&state_def.entry)?;
                state_config.push_str(
                    &entry_actions_code.replace("// Action:", "        s.on_entry(/* Action: */"),
                ); // Basic wrapping
            }
            if !state_def.exit.is_empty() {
                let exit_actions_code = generate_rust_actions(&state_def.exit)?;
                state_config.push_str(
                    &exit_actions_code.replace("// Action:", "        s.on_exit(/* Action: */"),
                ); // Basic wrapping
            }

            state_config.push_str("    });\n");
            machine_code.push_str(&state_config);
        }
    }

    machine_code.push_str("    builder.build()\n}");

    Ok(machine_code)
}

#[cfg(feature = "proto")]
fn generate_grpc_code<S, E, C>(machine: &Machine<S, E, C>) -> Result<String> {
    // Placeholder implementation for gRPC code generation
    // This would involve generating .proto files and potentially server/client stubs
    println!(
        "Warning: gRPC code generation (generate_grpc_code) for machine '{}' is not yet implemented.",
        machine.name
    );
    Ok("// gRPC code generation placeholder".to_string())
    // Actual implementation would need bounds on S, E, C for proto generation
    // Example bounds (adjust as needed for proto library):
    // S: StateTrait + Serialize + ProtoSerialize,
    // E: EventTrait + Serialize + ProtoSerialize,
    // C: Serialize + DeserializeOwned + Clone + Default + Send + Sync + Debug + 'static + ProtoSerialize,
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
        + Display
        + Default, // Added Default
    C: Clone + Debug + Default + Send + Sync + 'static + Serialize + DeserializeOwned, // Removed ContextTrait
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
    S: StateTrait + Display, // Need Display for state IDs in struct
    E: EventTrait + Display, // Need Display for event IDs in struct
    C: Clone + Debug + Default + Send + Sync + 'static + Serialize + DeserializeOwned,
{
    let state_enum_name = format!("{}State", machine_name); // Assuming State enum is generated elsewhere
    let context_type_name_str = std::any::type_name::<C>(); // Get the name of the context type

    // Sanitize machine_name for struct name (simple version)
    let struct_name = machine_name.replace(|c: char| !c.is_alphanumeric(), "-"); // Use &str
    let struct_name = struct_name.replace("-", "_"); // Use &str, ensure valid identifier
    let struct_name = format!("{}Machine", struct_name); // Append Machine

    // Placeholder: Generate fields based on states, context, etc.
    // This requires introspection into the builder's structure which isn't fully available here.
    let mut code = String::new();
    code.push_str(&format!(
        "struct {}Machine {{ /* ... fields ... */ }}",
        struct_name
    ));
    Ok(code)
}

pub fn generate_state_enum_code<
    S: StateTrait + Display + Debug + Clone + Eq + Hash + Serialize + DeserializeOwned,
>(
    state_ids: &[S], // Or just IDs
) -> String {
    println!("Warning: generate_state_enum_code is a placeholder.");
    let variants = state_ids
        .iter()
        .map(|s| format!("    {},", s))
        .collect::<Vec<_>>()
        .join(
            "
",
        );
    // Use a sanitized name like StateEnum to avoid conflicts with crate::State
    format!(
        "#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum StateEnum {{
{}
}}",
        variants
    ) // Removed trailing newline after {}
}

pub fn generate_event_enum_code<
    E: EventTrait + Display + Debug + Clone + Eq + Hash + Serialize + DeserializeOwned,
>(
    event_ids: &[E], // Or just IDs/names
) -> String {
    println!("Warning: generate_event_enum_code is a placeholder.");
    let variants = event_ids
        .iter()
        .map(|e| format!("    {},", e))
        .collect::<Vec<_>>()
        .join(
            "
",
        );
    // Use a sanitized name like EventEnum to avoid conflicts with crate::Event
    format!(
        "#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EventEnum {{
{}
}}",
        variants
    ) // Removed trailing newline after {}
}

pub fn generate_context_struct_code<
    C: Clone + Debug + Default + Serialize + DeserializeOwned + Send + Sync + 'static, // Removed ContextTrait
>(
    context_type_name: &str, // Name of the context type
) -> String {
    println!("Warning: generate_context_struct_code is a placeholder.");
    // Ensure context_type_name is a valid Rust identifier
    let struct_name = context_type_name.split("::").last().unwrap_or("Context"); // Basic sanitization
    format!(
        "#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct {} {{ /* ... fields ... */ }}
",
        struct_name
    )
}

pub fn generate_action_enum_code<E>(action_names: &[String]) -> String
where
    // C bounds removed as C is not used here
    E: EventTrait
        // Keep only necessary bounds for E if action names depend on it (currently they don't seem to)
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
    let variants = action_names
        .iter()
        .map(|a| format!("    {},", a))
        .collect::<Vec<_>>()
        .join(
            "
",
        );
    // Use a sanitized name like ActionEnum to avoid conflicts with crate::Action
    format!(
        "#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ActionEnum {{
{}
}}
",
        variants
    )
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
        + Display
        + Default, // Added Default
    C: Clone + Debug + Default + Send + Sync + 'static + Serialize + DeserializeOwned, // Added ContextTrait
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
    // generate_rust_code(builder) // This seems incorrect here, should return the error.
    // Corrected: Create String directly using String::from
    let err_msg = String::from("Not implemented");
    Err(Error::InvalidConfiguration(err_msg))
}

impl<C, E, S, O> MachineBuilder<C, E, S, O>
where
    S: StateTrait + Display + Clone + Eq + Hash + Send + Sync + 'static + From<String>,
    E: EventTrait
        + Clone
        + Eq
        + Hash
        + Send
        + Sync
        + 'static
        + Debug
        + IntoEvent
        + Display
        + Serialize
        + Default
        + DeserializeOwned,
    C: Clone + Debug + Default + Send + Sync + 'static + Serialize + DeserializeOwned,
    O: Default + Clone + Debug + Send + Sync + 'static + Serialize + DeserializeOwned,
{
    // Helper to get all state IDs, preserving order might be nice but HashMap doesn't guarantee it.
    pub fn get_state_ids(&self) -> Vec<S> {
        self.states.all().map(|s| s.id.clone()).collect()
    }
    // Helper to get all unique event types mentioned in transitions
    fn get_event_ids(&self) -> Vec<E> {
        let event_ids = HashSet::new();
        for state in self.states.all() {
            for transitions in state.on.values() {
                for transition in transitions {
                    if let Some(event_desc) = &transition.event {
                        // Assuming EventTrait allows conversion or access to the event identifier
                        // This needs adjustment based on how EventTrait works.
                        // If EventTrait itself is the ID (like an enum), clone it.
                        // If it wraps an ID, extract it.
                        // Placeholder: Need actual event ID extraction/cloning
                        // Let's assume EventTrait needs Clone + Eq + Hash
                        // And that the event descriptor *is* the event ID type E
                        // Attempt to get the event ID from the descriptor string
                        // E::from_description is not a standard part of EventTrait.
                        // This logic needs rework based on how event IDs are derived/stored.
                        // For now, comment out the problematic part.
                        // if let Ok(event_id) = E::from_description(event_desc) {
                        //     event_ids.insert(event_id);
                        // } else {
                        //     eprintln!(
                        //         "Warning: Could not convert event description '{}' to event ID.",
                        //         event_desc
                        //     );
                        // }
                    }
                }
            }
        }
        event_ids.into_iter().collect() // Collect unique event IDs
    }

    // Renamed and adjusted return type
    pub fn get_state(&self, state_id: &S) -> Option<&crate::state::State<S, C, E>> {
        self.states.get(state_id)
    }
}
