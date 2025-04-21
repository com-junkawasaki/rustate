#!/bin/bash
# RuState 統合テストスクリプト
# このスクリプトは rustate と rustate-grpc の統合テストを実行します

set -e

echo "=== RuState 統合テスト開始 ==="

# クレートのビルドと単体テスト
echo "-- rustate コアクレートのテスト --"
cd crates/rustate
cargo test --all-features
cd ../..

echo "-- rustate-grpc クレートのテスト --"
cd crates/rustate-grpc
cargo test --all-features
cd ../..

# 統合テスト
echo "-- 統合サンプルのビルドと実行 --"
cd crates/rustate-grpc
cargo build --example integration_combined_demo --features="full"
echo "統合サンプルのビルドが成功しました"

echo "すべてのテストは正常に完了しました！"