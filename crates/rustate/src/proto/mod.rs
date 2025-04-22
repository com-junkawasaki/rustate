// Include the generated Protocol Buffer code
pub mod xstate_machine {
    tonic::include_proto!("rustate.xstate");
}

mod converter;
pub use converter::*;

/// Error types for proto operations
#[derive(thiserror::Error, Debug)]
pub enum ProtoError {
    #[error("Invalid state machine definition: {0}")]
    InvalidDefinition(String),
    
    #[error("Failed to convert from proto format: {0}")]
    ConversionError(String),
    
    #[error("Protocol Buffer error: {0}")]
    ProtobufError(#[from] prost::DecodeError),
    
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    
    #[error("JSON error: {0}")]
    JsonError(#[from] serde_json::Error),
}

/// Result type for proto operations
pub type Result<T> = std::result::Result<T, ProtoError>;

/// Import a state machine from a Protocol Buffer definition
pub fn import_machine_from_proto(
    proto_bytes: &[u8]
) -> Result<crate::Machine> {
    let proto_machine = xstate_machine::ImportMachineRequest::decode(proto_bytes)?;
    converter::convert_from_proto(proto_machine)
}

/// Import a state machine from a Protocol Buffer file
pub fn import_machine_from_file(
    file_path: &str
) -> Result<crate::Machine> {
    let proto_bytes = std::fs::read(file_path)?;
    import_machine_from_proto(&proto_bytes)
}

/// Export a state machine to Protocol Buffer format
pub fn export_machine_to_proto(
    machine: &crate::Machine
) -> Result<Vec<u8>> {
    let proto_machine = converter::convert_to_proto(machine)?;
    let mut bytes = Vec::new();
    
    // Create a request with the machine definition
    let request = xstate_machine::ImportMachineRequest {
        definition: Some(proto_machine),
        options: None,
    };
    
    // Encode the request to bytes
    bytes.reserve(request.encoded_len());
    prost::Message::encode(&request, &mut bytes).map_err(|e| {
        ProtoError::ConversionError(format!("Failed to encode machine: {}", e))
    })?;
    
    Ok(bytes)
}

/// Export a state machine to a Protocol Buffer file
pub fn export_machine_to_file(
    machine: &crate::Machine,
    file_path: &str
) -> Result<()> {
    let bytes = export_machine_to_proto(machine)?;
    std::fs::write(file_path, bytes)?;
    Ok(())
} 