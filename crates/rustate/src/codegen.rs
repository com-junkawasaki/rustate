//! Code generation from state machine definitions.
//!
//! This module provides functionality to export `Machine` definitions into
//! other formats like JSON or Protocol Buffers schemas.
//!
//! Requires the `codegen` feature flag. Protocol Buffers export specifically
//! requires the `proto` feature flag in addition to `codegen`.

use crate::{
    action::Action,
    context::Context,
    error::Result,
    event::{Event, EventTrait},
    guard::Guard,
    machine::{Machine, MachineBuilder},
    state::{State, StateTrait},
    transition::Transition,
};
use serde::Serialize;
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;

// Conditionally import items needed for specific features
#[cfg(feature = "codegen")]
use {
    // proc_macro2::TokenStream, // Currently unused
    // quote::quote, // Currently unused
    syn::parse_file, // Used in parse_from_rust_file
                     // syn::Item, // Currently unused
};

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

impl CodegenExt for Machine {
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
        let machine = crate::MachineBuilder::<String, String, ()>::new("idle".to_string())
            .state("idle".to_string(), |s| {
                s.on("START".to_string(), |t| t.target("active".to_string()))
            })
            .state("active".to_string(), |s| {
                s.on("STOP".to_string(), |t| t.target("idle".to_string()))
            })
            // .initial("idle".to_string()) // Assuming new sets initial
            .build()?;

        Ok(machine)
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
pub fn machine_builder_to_json<S, E, C>(
    builder: &mut crate::MachineBuilder<S, E, C>,
) -> Result<String>
where
    S: crate::StateTrait + Send + Sync + Clone + 'static + Serialize, // Need Serialize
    E: crate::EventTrait + Send + Sync + Clone + 'static + Serialize, // Need Serialize
    C: crate::Context + Send + Sync + Clone + 'static + Serialize,    // Need Serialize
{
    // Clone the builder to avoid consuming the original, then build.
    let machine = builder.clone().build()?;
    Ok(serde_json::to_string_pretty(&machine)?)
}

/// Helper function to generate a basic Protocol Buffers schema from a `MachineBuilder`.
/// **Note:** Builds the machine internally but currently ignores its structure
/// and generates a static schema using `generate_proto_schema`.
/// Requires the `codegen` and `proto` features.
#[cfg(all(feature = "codegen", feature = "proto"))]
pub fn machine_builder_to_proto<
    S: crate::StateTrait + Send + Sync + Clone + 'static + Serialize, // Traits needed for build()
    E: crate::EventTrait + Send + Sync + Clone + 'static + Serialize,
    C: crate::Context + Send + Sync + Clone + 'static + Serialize,
>(
    _builder: &mut crate::MachineBuilder<S, E, C>, // Builder isn't actually used yet
    options: Option<ProtoExportOptions>,
) -> Result<String> {
    // let _machine = builder.clone().build()?; // Build machine if needed for future inspection
    println!("Warning: machine_builder_to_proto currently generates a static schema.");
    let opts = options.unwrap_or_default();
    generate_proto_schema(&opts)
}

#[cfg(feature = "codegen")]
fn generate_rust_actions<S, E, C>(
    actions: &[Action<C, E>],
    // Correct bounds for C
) -> Result<String>
where
    S: StateTrait + Send + Sync + Clone + 'static + Serialize,
    E: EventTrait + Send + Sync + Clone + 'static + Serialize,
    C: Clone + Debug + Default + Send + Sync + 'static + Serialize, // Corrected bounds for C
{
    // ... function body
    Ok(String::new()) // Placeholder return
}

#[cfg(feature = "codegen")]
fn generate_rust_guards<S, E, C>(
    guards: &[Guard<C, E>],
    // Correct bounds for C
) -> Result<String>
where
    S: StateTrait + Send + Sync + Clone + 'static + Serialize,
    E: EventTrait + Send + Sync + Clone + 'static + Serialize,
    C: Clone + Debug + Default + Send + Sync + 'static + Serialize, // Corrected bounds for C
{
    // ... function body
    Ok(String::new()) // Placeholder return
}

#[cfg(feature = "codegen")]
fn generate_rust_transitions<S, E, C>(
    transitions: &[Transition<S, C, E>],
    machine_name: &str,
    // Correct bounds for C
) -> Result<String>
where
    S: StateTrait + Send + Sync + Clone + 'static + Serialize,
    E: EventTrait + Send + Sync + Clone + 'static + Serialize,
    C: Clone + Debug + Default + Send + Sync + 'static + Serialize, // Corrected bounds for C
{
    // ... function body
    Ok(String::new()) // Placeholder return
}

#[cfg(feature = "codegen")]
pub fn generate_rust_code<S, E, C, O>(
    builder: &MachineBuilder<C, E, S, O>,
    // Correct bounds for C
) -> Result<String>
where
    S: StateTrait + Send + Sync + Clone + 'static + Serialize,
    E: EventTrait + Send + Sync + Clone + 'static + Serialize,
    C: Clone + Debug + Default + Send + Sync + 'static + Serialize, // Corrected bounds for C
    O: Clone + Debug + Default + Send + Sync + 'static + Serialize, // Assuming O needs Serialize too
{
    // ... function body
    Ok(String::new()) // Placeholder return
}
