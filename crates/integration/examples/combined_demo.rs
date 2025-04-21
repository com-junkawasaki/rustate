use rustate::{Action, ActionType, Event, Machine, MachineBuilder, State, Transition};
use rustate_integration::{
    event_forwarding::SharedMachineRef,
    context_sharing::SharedContext,
    hierarchical::{ChildMachine, DefaultChildMachine, coordination},
};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

fn main() -> rustate_integration::Result<()> {
    println!("= RuState クロスクレート統合デモ =");
    println!("3つの統合パターンを組み合わせた複合システムを実演します。\n");
    
    // 共有コンテキストを作成
    let shared_context = SharedContext::new();
    
    // ワークフローステートマシンを作成（親マシン）
    let workflow_machine = create_workflow_machine();
    let workflow = SharedMachineRef::new(workflow_machine);
    
    // プロセスAステートマシンを作成（コンテキスト共有パターン）
    let process_a = create_process_a(shared_context.clone());
    let process_a_ref = SharedMachineRef::new(process_a);
    
    // プロセスBステートマシンを作成（階層的パターン）
    let process_b = create_process_b();
    let child_machine = DefaultChildMachine::new(process_b, "completed");
    let child_machine = Arc::new(Mutex::new(child_machine));
    let process_b_controller = create_process_b_controller(child_machine.clone(), shared_context.clone());
    let process_b_controller_ref = SharedMachineRef::new(process_b_controller);
    
    // ワークフローからプロセスAへの接続（イベント転送パターン）
    connect_workflow_to_process_a(workflow.clone(), process_a_ref)?;
    
    // ワークフローからプロセスBコントローラへの接続（イベント転送パターン）
    connect_workflow_to_process_b(workflow.clone(), process_b_controller_ref)?;
    
    // システム全体を開始
    println!("ワークフローを開始します...");
    workflow.send_event("START")?;
    
    // 状態を表示するスレッド
    thread::spawn(move || {
        for i in 0..5 {
            thread::sleep(Duration::from_secs(1));
            
            println!("\n--- システム状態（{}秒後） ---", i + 1);
            
            if let Ok(is_in_process_a) = workflow.is_in("processA") {
                if is_in_process_a {
                    println!("ワークフロー: プロセスA実行中");
                }
            }
            
            if let Ok(is_in_process_b) = workflow.is_in("processB") {
                if is_in_process_b {
                    println!("ワークフロー: プロセスB実行中");
                }
            }
            
            if let Ok(is_completed) = workflow.is_in("completed") {
                if is_completed {
                    println!("ワークフロー: 完了");
                }
            }
            
            // 共有コンテキストの値を表示
            if let Ok(Some(status)) = shared_context.get::<String>("processA.status") {
                println!("共有コンテキスト: processA.status = {}", status);
            }
            
            if let Ok(Some(progress)) = shared_context.get::<i32>("processB.progress") {
                println!("共有コンテキスト: processB.progress = {}%", progress);
            }
            
            if let Ok(child) = child_machine.lock() {
                println!(
                    "プロセスB: 状態 = {:?}, 完了 = {}",
                    child.current_states(),
                    child.is_in_final_state()
                );
            }
            
            if i == 1 {
                // プロセスAを進める
                let _ = process_a_ref.send_event("NEXT");
            }
            
            if i == 3 {
                // プロセスBを完了させる
                if let Ok(mut child) = child_machine.lock() {
                    let _ = child.handle_parent_event("COMPLETE");
                }
            }
        }
    });
    
    // メインスレッドは少し待ってから終了
    thread::sleep(Duration::from_secs(6));
    println!("\nデモ完了");
    
    Ok(())
}

/// ワークフローステートマシンを作成
fn create_workflow_machine() -> Machine {
    // 状態
    let initial = State::new("initial");
    let process_a = State::new("processA");
    let process_b = State::new("processB");
    let completed = State::new("completed");
    
    // 遷移
    let start = Transition::new("initial", "START", "processA");
    let a_to_b = Transition::new("processA", "NEXT", "processB");
    let complete = Transition::new("processB", "COMPLETE", "completed");
    
    MachineBuilder::new("workflow")
        .state(initial)
        .state(process_a)
        .state(process_b)
        .state(completed)
        .initial("initial")
        .transition(start)
        .transition(a_to_b)
        .transition(complete)
        .build()
        .unwrap()
}

/// プロセスAステートマシンを作成（コンテキスト共有パターンを使用）
fn create_process_a(context: SharedContext) -> Machine {
    // 状態
    let waiting = State::new("waiting");
    let running = State::new("running");
    let done = State::new("done");
    
    // 遷移
    let start = Transition::new("waiting", "START", "running");
    let next = Transition::new("running", "NEXT", "done");
    
    // アクション - 共有コンテキストを更新
    let update_context = Action::new(
        "updateContext",
        ActionType::Entry,
        move |_ctx, evt| {
            let status = match evt.event_type.as_str() {
                "START" => "running",
                "NEXT" => "completed",
                _ => "unknown",
            };
            let _ = context.set("processA.status", status);
        },
    );
    
    MachineBuilder::new("processA")
        .state(waiting)
        .state(running)
        .state(done)
        .initial("waiting")
        .transition(start)
        .transition(next)
        .on_entry("running", update_context.clone())
        .on_entry("done", update_context)
        .build()
        .unwrap()
}

/// プロセスBステートマシンを作成（階層的パターンの子マシン）
fn create_process_b() -> Machine {
    // 状態
    let initial = State::new("initial");
    let processing = State::new("processing");
    let completed = State::new_final("completed");
    
    // 遷移
    let start = Transition::new("initial", "START", "processing");
    let complete = Transition::new("processing", "COMPLETE", "completed");
    
    MachineBuilder::new("processB")
        .state(initial)
        .state(processing)
        .state(completed)
        .initial("initial")
        .transition(start)
        .transition(complete)
        .build()
        .unwrap()
}

/// プロセスBコントローラステートマシンを作成（階層的パターンの親マシン）
fn create_process_b_controller(
    child: Arc<Mutex<impl ChildMachine + 'static>>,
    context: SharedContext,
) -> Machine {
    // 状態
    let monitoring = State::new("monitoring");
    let completed = State::new("completed");
    
    // 遷移
    let complete = Transition::new("monitoring", "COMPLETE", "completed");
    
    // 子マシンの状態を監視するアクション
    let monitor_child = coordination::create_child_monitor_action(
        "monitorChild",
        child.clone(),
    );
    
    // 子マシンにイベントを転送するアクション
    let start_child = coordination::create_event_forwarder_action(
        "startChild",
        child.clone(),
        "START",
        "START",
    );
    
    // 進捗状況を更新するアクション
    let update_progress = Action::new(
        "updateProgress",
        ActionType::Transition,
        move |_ctx, _evt| {
            if let Ok(child) = child.lock() {
                if child.is_in("processing") {
                    let _ = context.set("processB.progress", 50);
                } else if child.is_in_final_state() {
                    let _ = context.set("processB.progress", 100);
                } else {
                    let _ = context.set("processB.progress", 0);
                }
            }
        },
    );
    
    // 子マシンが完了したことを確認するガード
    let child_completed = ("childCompleted", move |_: &rustate::Context, _: &Event| {
        if let Ok(child) = child.lock() {
            child.is_in_final_state()
        } else {
            false
        }
    });
    
    MachineBuilder::new("processBController")
        .state(monitoring)
        .state(completed)
        .initial("monitoring")
        .on_entry("monitoring", monitor_child)
        .on_entry("monitoring", start_child)
        .on_entry("monitoring", update_progress.clone())
        .transition(complete.with_guard(child_completed))
        .build()
        .unwrap()
}

/// ワークフローからプロセスAへの接続
fn connect_workflow_to_process_a(
    workflow: SharedMachineRef,
    process_a: SharedMachineRef,
) -> rustate_integration::Result<Machine> {
    // ワークフローの状態
    let state = State::new("connector");
    
    // ワークフローからの状態変更を監視するアクション
    let monitor_workflow = Action::new(
        "monitorWorkflow",
        ActionType::Transition,
        move |_ctx, _evt| {
            // ワークフローがプロセスA状態になったらプロセスAを開始
            if let Ok(true) = workflow.is_in("processA") {
                let _ = process_a.send_event("START");
            }
        },
    );
    
    // コネクタマシンを構築
    let machine = MachineBuilder::new("workflowToProcessAConnector")
        .state(state)
        .initial("connector")
        .on_entry("connector", monitor_workflow)
        .build()?;
        
    Ok(machine)
}

/// ワークフローからプロセスBへの接続
fn connect_workflow_to_process_b(
    workflow: SharedMachineRef,
    process_b: SharedMachineRef,
) -> rustate_integration::Result<Machine> {
    // ワークフローの状態
    let state = State::new("connector");
    
    // ワークフローからの状態変更を監視するアクション
    let monitor_workflow = Action::new(
        "monitorWorkflow",
        ActionType::Transition,
        move |_ctx, _evt| {
            // ワークフローがプロセスB状態になったらプロセスBを開始
            if let Ok(true) = workflow.is_in("processB") {
                let _ = process_b.send_event("START");
            }
            
            // プロセスBが完了したらワークフローを進める
            if let Ok(true) = process_b.is_in("completed") {
                let _ = workflow.send_event("COMPLETE");
            }
        },
    );
    
    // コネクタマシンを構築
    let machine = MachineBuilder::new("workflowToProcessBConnector")
        .state(state)
        .initial("connector")
        .on_entry("connector", monitor_workflow)
        .build()?;
        
    Ok(machine)
} 