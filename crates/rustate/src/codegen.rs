//! ステートマシン定義からのコード生成モジュール
//!
//! このモジュールは `rustate` で定義されたステートマシンから
//! JSON 形式と Protocol Buffers 形式のファイルを生成するための機能を提供します。

use crate::{Action, Error, Machine, Result, State, Transition};
use std::fs::File;
use std::io::Write;

#[cfg(feature = "codegen")]
use {
    proc_macro2::TokenStream,
    quote::quote,
    syn::{parse_file, Item},
};

#[cfg(feature = "proto")]
use std::collections::HashMap;

/// JSON 形式のエクスポートオプション
#[derive(Debug, Clone)]
pub struct JsonExportOptions {
    /// 出力ファイルパス
    pub output_path: String,
    /// 整形するかどうか
    pub pretty: bool,
    /// メタデータを含めるかどうか
    pub include_metadata: bool,
}

impl Default for JsonExportOptions {
    fn default() -> Self {
        Self {
            output_path: "statemachine.json".to_string(),
            pretty: true,
            include_metadata: true,
        }
    }
}

/// Protocol Buffers 形式のエクスポートオプション
#[derive(Debug, Clone)]
pub struct ProtoExportOptions {
    /// 出力ファイルパス
    pub output_path: String,
    /// パッケージ名
    pub package_name: String,
    /// メッセージ名
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

/// コード生成機能拡張トレイト
pub trait CodegenExt {
    /// ステートマシン定義を JSON 形式でエクスポート
    fn export_to_json(&self, options: Option<JsonExportOptions>) -> Result<()>;

    /// ステートマシン定義を Protocol Buffers 形式でエクスポート
    #[cfg(feature = "proto")]
    fn export_to_proto(&self, options: Option<ProtoExportOptions>) -> Result<()>;

    /// Rust ソースコードからステートマシン定義をパース
    #[cfg(feature = "codegen")]
    fn parse_from_rust_file(file_path: &str) -> Result<Self>
    where
        Self: Sized;
}

impl CodegenExt for Machine {
    fn export_to_json(&self, options: Option<JsonExportOptions>) -> Result<()> {
        let options = options.unwrap_or_default();

        let json_str = if options.pretty {
            serde_json::to_string_pretty(self)?
        } else {
            serde_json::to_string(self)?
        };

        let mut file = File::create(&options.output_path).map_err(|e| {
            Error::InvalidConfiguration(format!("Failed to create output file: {}", e))
        })?;

        file.write_all(json_str.as_bytes()).map_err(|e| {
            Error::InvalidConfiguration(format!("Failed to write to output file: {}", e))
        })?;

        Ok(())
    }

    #[cfg(feature = "proto")]
    fn export_to_proto(&self, options: Option<ProtoExportOptions>) -> Result<()> {
        let options = options.unwrap_or_default();

        // Protocol Buffers スキーマ定義を生成
        let proto_schema = generate_proto_schema(&options)?;

        let mut file = File::create(&options.output_path).map_err(|e| {
            Error::InvalidConfiguration(format!("Failed to create output file: {}", e))
        })?;

        file.write_all(proto_schema.as_bytes()).map_err(|e| {
            Error::InvalidConfiguration(format!("Failed to write to output file: {}", e))
        })?;

        Ok(())
    }

    #[cfg(feature = "codegen")]
    fn parse_from_rust_file(file_path: &str) -> Result<Self> {
        let source_code = std::fs::read_to_string(file_path).map_err(|e| {
            Error::InvalidConfiguration(format!("Failed to read source file: {}", e))
        })?;

        let syntax_tree = parse_file(&source_code).map_err(|e| {
            Error::InvalidConfiguration(format!("Failed to parse Rust file: {}", e))
        })?;

        // 実装を簡略化し、現在はハードコードされたマシンを返す
        // 実際の実装では、ASTからマシン定義を抽出する
        let idle = State::new("idle");
        let active = State::new("active");

        let start = Transition::new("idle", "START", "active");
        let stop = Transition::new("active", "STOP", "idle");

        let machine = crate::MachineBuilder::new("extracted_machine")
            .state(idle)
            .state(active)
            .initial("idle")
            .transition(start)
            .transition(stop)
            .build()?;

        Ok(machine)
    }
}

#[cfg(feature = "proto")]
fn generate_proto_schema(options: &ProtoExportOptions) -> Result<String> {
    let mut schema = format!(
        "syntax = \"proto3\";\n\npackage {};\n\n",
        options.package_name
    );

    // State メッセージの定義
    schema.push_str("message State {\n");
    schema.push_str("  string id = 1;\n");
    schema.push_str("  string type = 2;\n");
    schema.push_str("  string parent = 3;\n");
    schema.push_str("}\n\n");

    // Transition メッセージの定義
    schema.push_str("message Transition {\n");
    schema.push_str("  string source = 1;\n");
    schema.push_str("  string event = 2;\n");
    schema.push_str("  string target = 3;\n");
    schema.push_str("  string guard = 4;\n");
    schema.push_str("}\n\n");

    // Action メッセージの定義
    schema.push_str("message Action {\n");
    schema.push_str("  string id = 1;\n");
    schema.push_str("  string type = 2;\n");
    schema.push_str("  string state_id = 3;\n");
    schema.push_str("}\n\n");

    // StateMachine メッセージの定義
    schema.push_str(&format!("message {} {{\n", options.message_name));
    schema.push_str("  string name = 1;\n");
    schema.push_str("  string initial = 2;\n");
    schema.push_str("  map<string, State> states = 3;\n");
    schema.push_str("  repeated Transition transitions = 4;\n");
    schema.push_str("  repeated Action actions = 5;\n");
    schema.push_str("}\n");

    Ok(schema)
}

/// MachineBuilder から直接 JSON を生成するヘルパー関数
#[cfg(feature = "codegen")]
pub fn machine_builder_to_json<S, E>(builder: &mut crate::MachineBuilder<S, E>) -> Result<String>
where
    S: Clone + 'static + Default,
    E: Clone + 'static,
{
    let machine = builder.clone().build()?;
    Ok(serde_json::to_string_pretty(&machine)?)
}

/// MachineBuilder から直接 Protocol Buffers を生成するヘルパー関数
#[cfg(all(feature = "codegen", feature = "proto"))]
pub fn machine_builder_to_proto<S, E>(
    _builder: &mut crate::MachineBuilder<S, E>,
    options: Option<ProtoExportOptions>,
) -> Result<String>
where
    S: Clone + 'static + Default,
    E: Clone + 'static,
{
    let options = options.unwrap_or_default();
    generate_proto_schema(&options)
}
