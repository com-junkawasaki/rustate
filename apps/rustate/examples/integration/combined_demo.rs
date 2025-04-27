use rustate::{
    // 統合パターン機能を使用
    integration::{
        context_sharing::SharedContext,
        event_forwarding::SharedMachineRef,
        hierarchical::{coordination, ChildMachine, DefaultChildMachine},
        Result,
    },
    Action,
    ActionType,
    Event,
    Machine,
    MachineBuilder,
    State,
    Transition,
};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

fn main() -> Result<()> {
    println!("= RuState 統合パターンデモ =");
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
    let process_b_controller =
        create_process_b_controller(child_machine.clone(), shared_context.clone());
    let process_b_controller_ref = SharedMachineRef::new(process_b_controller);

    // ワークフローからプロセスAへの接続（イベント転送パターン）
    connect_workflow_to_process_a(workflow.clone(), process_a_ref.clone())?;

    // ワークフローからプロセスBコントローラへの接続（イベント転送パターン）
    connect_workflow_to_process_b(workflow.clone(), process_b_controller_ref)?;

    // システム全体を開始
    println!("ワークフローを開始します...");
    workflow.send_event("START")?;

    // 状態を表示するスレッド
    thread::spawn(move || {
        for i in 0..8 {
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
                println!("\n>>> プロセスAにSTARTイベントを送信");
                // プロセスAを開始
                let _ = process_a_ref.send_event("START");
            }

            if i == 2 {
                println!("\n>>> プロセスAにNEXTイベントを送信");
                // プロセスAを進める
                let _ = process_a_ref.send_event("NEXT");
            }

            if i == 3 {
                println!("\n>>> ワークフローにNEXTイベントを送信");
                // ワークフローを次のステップに進める
                let _ = workflow.send_event("NEXT");
            }

            if i == 5 {
                println!("\n>>> プロセスBにSTARTイベントを送信");
                // プロセスBを開始
                if let Ok(mut child) = child_machine.lock() {
                    let _ = child.handle_parent_event("START");
                }
            }

            if i == 6 {
                println!("\n>>> プロセスBにCOMPLETEイベントを送信");
                // プロセスBを完了させる
                if let Ok(mut child) = child_machine.lock() {
                    let _ = child.handle_parent_event("COMPLETE");
                }
            }
        }
    });

    // メインスレッドは少し待ってから終了
    thread::sleep(Duration::from_secs(9));
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
    let update_context = Action::new("updateContext", ActionType::Entry, move |_ctx, evt| {
        let status = match evt.event_type.as_str() {
            "START" => "running",
            "NEXT" => "completed",
            _ => "unknown",
        };
        let _ = context.set("processA.status", status);
    });

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

    // ガード用のクローン
    let child_for_guard = child.clone();
    // アクション用のクローン
    let child_for_action = child.clone();
    // 子マシンの状態を監視するアクション
    let monitor_child = coordination::create_child_monitor_action("monitorChild", child.clone());

    // 子マシンにイベントを転送するアクション
    let start_child =
        coordination::create_event_forwarder_action("startChild", child.clone(), "START", "START");

    // 子マシンが完了したことを確認するガード
    let child_completed = ("childCompleted", move |_: &rustate::Context, _: &Event| {
        if let Ok(child) = child_for_guard.lock() {
            child.is_in_final_state()
        } else {
            false
        }
    });

    // 進捗状況を更新するアクション
    let update_progress = Action::new("updateProgress", ActionType::Transition, move |_ctx, _evt| {
        // 進捗状況を更新
        if let Ok(child) = child_for_action.lock() {
            if child.is_in("processing") {
                for i in (0..=100).step_by(20) {
                    let _ = context.set("processB.progress", i);
                    thread::sleep(Duration::from_millis(100));
                }
            }
        }
    });

    // 完了イベントを親に送信するアクション（子→親）
    let notify_completion =
        Action::new("notifyCompletion", ActionType::Transition, |_ctx, _evt| {
            println!("プロセスBが完了し、親に通知します");
        });

    // 完了への遷移（内部遷移）
    let mut monitor_to_complete = Transition::new("monitoring", "*", "completed");
    monitor_to_complete
        .with_guard(child_completed)
        .with_action(notify_completion);

    MachineBuilder::new("processBController")
        .state(monitoring)
        .state(completed)
        .initial("monitoring")
        .transition(monitor_to_complete)
        .on_entry("monitoring", monitor_child)
        .on_entry("monitoring", start_child)
        .on_entry("monitoring", update_progress)
        .build()
        .unwrap()
}

/// ワークフローとプロセスAを接続（イベント転送パターン）
fn connect_workflow_to_process_a(
    workflow: SharedMachineRef,
    process_a: SharedMachineRef,
) -> Result<Machine> {
    // イベント転送用ステートマシン
    let initial = State::new("initial");
    let forwarder = State::new("forwarder");

    // アクション - ワークフローからプロセスAへのイベント転送
    let forward_event = Action::new(
        "forwardToProcessA",
        ActionType::Transition,
        move |_ctx, evt| {
            println!("ワークフローからプロセスAにイベントを転送: {}", evt.event_type);
            if evt.event_type == "START" {
                let _ = process_a.send_event("START");
            }
        },
    );

    // アクション - プロセスAの状態変化を監視
    let monitor_process_a = Action::new(
        "monitorProcessA",
        ActionType::Entry,
        move |_ctx, _evt| {
            // プロセスAの状態を定期的に監視する
            let process_a_ref = workflow.clone();
            thread::spawn(move || {
                loop {
                    thread::sleep(Duration::from_millis(500));
                    if let Ok(true) = process_a_ref.is_in("processA") {
                        if let Ok(true) = process_a.is_in("done") {
                            println!("プロセスAが完了を検出 - ワークフローにNEXTを送信");
                            let _ = process_a_ref.send_event("NEXT");
                            break;
                        }
                    } else {
                        break; // プロセスAが非アクティブになったら終了
                    }
                }
            });
        },
    );

    let start = Transition::new("initial", "START", "forwarder").with_action(forward_event);

    MachineBuilder::new("workflow_to_process_a")
        .state(initial)
        .state(forwarder)
        .initial("initial")
        .transition(start)
        .on_entry("forwarder", monitor_process_a)
        .build()
}

/// ワークフローとプロセスBを接続（イベント転送パターン）
fn connect_workflow_to_process_b(
    workflow: SharedMachineRef,
    process_b: SharedMachineRef,
) -> Result<Machine> {
    // イベント転送用ステートマシン
    let initial = State::new("initial");
    let forwarder = State::new("forwarder");

    // アクション - ワークフローからプロセスBへのイベント転送
    let forward_event = Action::new(
        "forwardToProcessB",
        ActionType::Transition,
        move |_ctx, evt| {
            println!("ワークフローからプロセスBにイベントを転送: {}", evt.event_type);
            if evt.event_type == "NEXT" {
                let _ = process_b.send_event("START");
            }
        },
    );

    // アクション - プロセスBの状態変化を監視
    let monitor_process_b = Action::new(
        "monitorProcessB",
        ActionType::Entry,
        move |_ctx, _evt| {
            // プロセスBの状態を定期的に監視する
            let process_b_ref = workflow.clone();
            thread::spawn(move || {
                loop {
                    thread::sleep(Duration::from_millis(500));
                    if let Ok(true) = process_b_ref.is_in("processB") {
                        if let Ok(true) = process_b.is_in("completed") {
                            println!("プロセスBが完了を検出 - ワークフローにCOMPLETEを送信");
                            let _ = process_b_ref.send_event("COMPLETE");
                            break;
                        }
                    } else {
                        break; // プロセスBが非アクティブになったら終了
                    }
                }
            });
        },
    );

    let start = Transition::new("initial", "NEXT", "forwarder").with_action(forward_event);

    MachineBuilder::new("workflow_to_process_b")
        .state(initial)
        .state(forwarder)
        .initial("initial")
        .transition(start)
        .on_entry("forwarder", monitor_process_b)
        .build()
}
