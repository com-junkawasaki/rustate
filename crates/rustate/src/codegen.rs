//! ステートマシン定義からのコード生成モジュール
//!
//! このモジュールは `rustate` で定義されたステートマシンから
//! JSON 形式と Protocol Buffers 形式のファイルを生成するための機能を提供します。

use crate::{Machine, Result, Error, State, Transition, Context, Action};
use std::fs::File;
use std::io::Write;
use std::path::Path;

#[cfg(feature = "codegen")]
use {
    proc_macro2::TokenStream,
    quote::{quote, format_ident},
    syn::{parse_file, Item, ItemStruct},
};

#[cfg(feature = "proto")]
use {
    prost::Message,
    std::collections::HashMap,
};

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
    fn parse_from_rust_file(file_path: &str) -> Result<Machine>;
}

impl CodegenExt for Machine {
    fn export_to_json(&self, options: Option<JsonExportOptions>) -> Result<()> {
        let options = options.unwrap_or_default();
        
        let json_str = if options.pretty {
            serde_json::to_string_pretty(self)?
        } else {
            serde_json::to_string(self)?
        };
        
        let mut file = File::create(&options.output_path)
            .map_err(|e| Error::InvalidConfiguration(format!("Failed to create output file: {}", e)))?;
            
        file.write_all(json_str.as_bytes())
            .map_err(|e| Error::InvalidConfiguration(format!("Failed to write to output file: {}", e)))?;
            
        Ok(())
    }
    
    #[cfg(feature = "proto")]
    fn export_to_proto(&self, options: Option<ProtoExportOptions>) -> Result<()> {
        let options = options.unwrap_or_default();
        
        // Protocol Buffers スキーマ定義を生成
        let proto_schema = generate_proto_schema(self, &options)?;
        
        let mut file = File::create(&options.output_path)
            .map_err(|e| Error::InvalidConfiguration(format!("Failed to create output file: {}", e)))?;
            
        file.write_all(proto_schema.as_bytes())
            .map_err(|e| Error::InvalidConfiguration(format!("Failed to write to output file: {}", e)))?;
            
        Ok(())
    }
    
    #[cfg(feature = "codegen")]
    fn parse_from_rust_file(file_path: &str) -> Result<Machine> {
        let source_code = std::fs::read_to_string(file_path)
            .map_err(|e| Error::InvalidConfiguration(format!("Failed to read source file: {}", e)))?;
            
        let syntax_tree = parse_file(&source_code)
            .map_err(|e| Error::InvalidConfiguration(format!("Failed to parse Rust file: {}", e)))?;
            
        let mut builder = None;
        
        // ファイル内のステートマシンビルダー定義を探す
        for item in syntax_tree.items {
            if let Item::Fn(item_fn) = item {
                // 関数本体からMachineBuilderの使用を探す
                if let Some(machine_builder) = extract_machine_builder(&item_fn.block) {
                    builder = Some(machine_builder);
                    break;
                }
            }
        }
        
        if let Some(builder) = builder {
            // 抽出したビルダー情報からMachineインスタンスを構築
            Ok(builder.build()?)
        } else {
            Err(Error::InvalidConfiguration("No state machine definition found in the file".into()))
        }
    }
}

#[cfg(feature = "proto")]
fn generate_proto_schema(machine: &Machine, options: &ProtoExportOptions) -> Result<String> {
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

#[cfg(feature = "codegen")]
fn extract_machine_builder(block: &syn::Block) -> Option<crate::MachineBuilder> {
    use syn::{Expr, ExprCall, ExprMethodCall, ExprPath, Path, PathSegment};
    
    // MachineBuilderのメソッドチェーンを探す
    for stmt in &block.stmts {
        if let syn::Stmt::Local(local) = stmt {
            if let Some((_, init)) = &local.init {
                if let Expr::MethodCall(method_chain) = &**init {
                    // buildメソッドで終わるメソッドチェーンを探す
                    if let Some(method_name) = method_chain.method.segments.last() {
                        if method_name.ident == "build" {
                            // メソッドチェーンを遡って解析
                            let mut builder = crate::MachineBuilder::new("extracted_machine");
                            
                            // ここでは実装を簡略化していますが、実際にはメソッドチェーン全体を
                            // 解析して、状態、遷移、アクションなどを抽出する必要があります
                            
                            return Some(builder);
                        }
                    }
                }
            }
        } else if let syn::Stmt::Expr(expr, _) = stmt {
            // 式文も確認
            if let Expr::MethodCall(method_chain) = expr {
                if let Some(method_name) = method_chain.method.segments.last() {
                    if method_name.ident == "build" {
                        // この場合も同様にメソッドチェーンを解析
                        let mut builder = crate::MachineBuilder::new("extracted_machine");
                        
                        // メソッドチェーンの詳細な解析（省略）
                        
                        return Some(builder);
                    }
                }
            }
        }
    }
    
    // サブブロック内も再帰的に探索
    for stmt in &block.stmts {
        match stmt {
            syn::Stmt::Expr(Expr::Block(expr_block), _) => {
                if let Some(builder) = extract_machine_builder(&expr_block.block) {
                    return Some(builder);
                }
            },
            syn::Stmt::Semi(Expr::Block(expr_block), _) => {
                if let Some(builder) = extract_machine_builder(&expr_block.block) {
                    return Some(builder);
                }
            },
            syn::Stmt::Local(local) => {
                if let Some((_, init)) = &local.init {
                    if let Expr::Block(expr_block) = &**init {
                        if let Some(builder) = extract_machine_builder(&expr_block.block) {
                            return Some(builder);
                        }
                    }
                }
            },
            syn::Stmt::Item(syn::Item::Fn(item_fn)) => {
                if let Some(builder) = extract_machine_builder(&item_fn.block) {
                    return Some(builder);
                }
            },
            // 他にもケースを追加
            _ => {}
        }
    }
    
    None
}

/// ASTから状態定義を抽出するヘルパー関数
#[cfg(feature = "codegen")]
fn extract_states(expr: &syn::Expr) -> Vec<State> {
    let mut states = Vec::new();
    
    // 実装省略: ASTから状態定義を抽出するロジック
    
    states
}

/// ASTから遷移定義を抽出するヘルパー関数
#[cfg(feature = "codegen")]
fn extract_transitions(expr: &syn::Expr) -> Vec<Transition> {
    let mut transitions = Vec::new();
    
    // 実装省略: ASTから遷移定義を抽出するロジック
    
    transitions
}

/// ASTからアクション定義を抽出するヘルパー関数
#[cfg(feature = "codegen")]
fn extract_actions(expr: &syn::Expr) -> Vec<Action> {
    let mut actions = Vec::new();
    
    // 実装省略: ASTからアクション定義を抽出するロジック
    
    actions
}

/// MachineBuilder から直接 JSON を生成するヘルパー関数
#[cfg(feature = "codegen")]
pub fn machine_builder_to_json<S, E>(builder: &crate::MachineBuilder<S, E>) -> Result<String>
where
    S: Clone + 'static + Default,
    E: Clone + 'static,
{
    let machine = builder.build()?;
    Ok(serde_json::to_string_pretty(&machine)?)
}

/// MachineBuilder から直接 Protocol Buffers を生成するヘルパー関数
#[cfg(all(feature = "codegen", feature = "proto"))]
pub fn machine_builder_to_proto<S, E>(
    builder: &crate::MachineBuilder<S, E>,
    options: Option<ProtoExportOptions>
) -> Result<String>
where
    S: Clone + 'static + Default,
    E: Clone + 'static,
{
    let machine = builder.build()?;
    let options = options.unwrap_or_default();
    generate_proto_schema(&machine, &options)
} 