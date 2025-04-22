fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_build::compile_protos("src/proto/xstate_machine.proto")?;
    println!("cargo:rerun-if-changed=src/proto/xstate_machine.proto");
    Ok(())
} 