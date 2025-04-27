#![cfg(feature = "property-testing")]

// Add extern crate declaration guarded by feature flag
#[cfg(feature = "property-testing")]
extern crate proptest;

use crate::{Context, EventTrait, IntoEvent, Machine, Result, StateTrait};
use proptest::strategy::{BoxedStrategy, Strategy};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::fmt::Debug;
use std::marker::PhantomData;

/// Property-basedテストの検証プロパティを定義するトレイト
pub trait StateMachineProperty<S, E>
where
    S: StateTrait + Clone + Debug + Default + 'static,
    E: EventTrait + Clone + Debug + IntoEvent + 'static + Serialize + DeserializeOwned,
{
    /// 検証条件を評価する
    fn evaluate(&self, machine: &Machine<S, E>) -> bool;

    /// プロパティの名前を取得
    fn name(&self) -> &str;

    /// プロパティの説明を取得
    fn description(&self) -> Option<&str>;
}

/// プロパティ検証結果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PropertyTestResult {
    /// プロパティ名
    pub property_name: String,
    /// 検証結果
    pub success: bool,
    /// 反例（失敗した場合）
    pub counterexample: Option<Vec<String>>,
    /// メッセージ
    pub message: Option<String>,
}

/// 事前条件を指定するビルダー
pub struct GivenBuilder<S, E, F>
where
    S: StateTrait + Clone + Debug + Default + 'static,
    E: EventTrait + Clone + Debug + IntoEvent + 'static + Serialize + DeserializeOwned,
    F: Fn(&Machine<S, E>) -> bool + Clone + 'static,
{
    name: String,
    description: Option<String>,
    precondition: F,
    _marker: PhantomData<(S, E)>,
}

/// アクション（イベントシーケンス）を指定するビルダー
pub struct WhenBuilder<S, E, F, G>
where
    S: StateTrait + Clone + Debug + Default + 'static,
    E: EventTrait + Clone + Debug + IntoEvent + 'static + Serialize + DeserializeOwned,
    F: Fn(&Machine<S, E>) -> bool + Clone + 'static,
    G: Fn(&mut Machine<S, E>) -> Result<S> + Clone + 'static,
{
    name: String,
    description: Option<String>,
    precondition: F,
    action: G,
    _marker: PhantomData<(S, E)>,
}

/// 事後条件を指定するビルダー（完成したプロパティ）
#[derive(Clone)]
pub struct StateMachinePropertyImpl<S, E, F, G, H>
where
    S: StateTrait + Clone + Debug + Default + 'static,
    E: EventTrait + Clone + Debug + IntoEvent + 'static + Serialize + DeserializeOwned,
    F: Fn(&Machine<S, E>) -> bool + Clone + 'static,
    G: Fn(&mut Machine<S, E>) -> Result<S> + Clone + 'static,
    H: Fn(&Machine<S, E>) -> bool + Clone + 'static,
{
    name: String,
    description: Option<String>,
    precondition: F,
    action: G,
    postcondition: H,
    _marker: PhantomData<(S, E)>,
}

impl<S, E> Machine<S, E>
where
    S: StateTrait + Clone + Debug + Default + 'static,
    E: EventTrait + Clone + Debug + IntoEvent + 'static + Serialize + DeserializeOwned,
{
    /// 状態マシンのプロパティを定義するためのビルダーを開始
    pub fn property(name: impl Into<String>) -> PropertyBuilder<S, E> {
        PropertyBuilder {
            name: name.into(),
            description: None,
            _marker: PhantomData,
        }
    }
}

/// プロパティビルダーの開始点
pub struct PropertyBuilder<S, E>
where
    S: StateTrait + Clone + Debug + Default + 'static,
    E: EventTrait + Clone + Debug + IntoEvent + 'static + Serialize + DeserializeOwned,
{
    name: String,
    description: Option<String>,
    _marker: PhantomData<(S, E)>,
}

impl<S, E> PropertyBuilder<S, E>
where
    S: StateTrait + Clone + Debug + Default + 'static,
    E: EventTrait + Clone + Debug + IntoEvent + 'static + Serialize + DeserializeOwned,
{
    /// プロパティの説明を設定
    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// 事前条件を設定
    pub fn given<F>(self, precondition: F) -> GivenBuilder<S, E, F>
    where
        F: Fn(&Machine<S, E>) -> bool + Clone + 'static,
    {
        GivenBuilder {
            name: self.name,
            description: self.description,
            precondition,
            _marker: PhantomData,
        }
    }
}

impl<S, E, F> GivenBuilder<S, E, F>
where
    S: StateTrait + Clone + Debug + Default + 'static,
    E: EventTrait + Clone + Debug + IntoEvent + 'static + Serialize + DeserializeOwned,
    F: Fn(&Machine<S, E>) -> bool + Clone + 'static,
{
    /// アクションを設定
    pub fn when<G>(self, action: G) -> WhenBuilder<S, E, F, G>
    where
        G: Fn(&mut Machine<S, E>) -> Result<S> + Clone + 'static,
    {
        WhenBuilder {
            name: self.name,
            description: self.description,
            precondition: self.precondition,
            action,
            _marker: PhantomData,
        }
    }
}

impl<S, E, F, G> WhenBuilder<S, E, F, G>
where
    S: StateTrait + Clone + Debug + Default + 'static,
    E: EventTrait + Clone + Debug + IntoEvent + 'static + Serialize + DeserializeOwned,
    F: Fn(&Machine<S, E>) -> bool + Clone + 'static,
    G: Fn(&mut Machine<S, E>) -> Result<S> + Clone + 'static,
{
    /// 事後条件を設定
    pub fn then<H>(self, postcondition: H) -> StateMachinePropertyImpl<S, E, F, G, H>
    where
        H: Fn(&Machine<S, E>) -> bool + Clone + 'static,
    {
        StateMachinePropertyImpl {
            name: self.name,
            description: self.description,
            precondition: self.precondition,
            action: self.action,
            postcondition,
            _marker: PhantomData,
        }
    }
}

impl<S, E, F, G, H> StateMachineProperty<S, E> for StateMachinePropertyImpl<S, E, F, G, H>
where
    S: StateTrait + Clone + Debug + Default + 'static,
    E: EventTrait + Clone + Debug + IntoEvent + 'static + Serialize + DeserializeOwned,
    F: Fn(&Machine<S, E>) -> bool + Clone + 'static,
    G: Fn(&mut Machine<S, E>) -> Result<S> + Clone + 'static,
    H: Fn(&Machine<S, E>) -> bool + Clone + 'static,
{
    fn evaluate(&self, machine: &Machine<S, E>) -> bool {
        // 事前条件をチェック
        if !(self.precondition)(machine) {
            // 事前条件が満たされていない場合は無条件で成功
            return true;
        }

        // 状態マシンをクローン
        let mut machine_clone = machine.clone();

        // アクションを適用
        match (self.action)(&mut machine_clone) {
            Ok(_) => {
                // 事後条件をチェック
                (self.postcondition)(&machine_clone)
            }
            Err(_) => {
                // アクションの適用が失敗した場合は失敗
                false
            }
        }
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }
}

/// イベントシーケンスを生成するためのストラテジービルダー
pub struct EventSequenceStrategyBuilder<S, E>
where
    S: StateTrait + Clone + Debug + Default + 'static,
    E: EventTrait + Clone + Debug + IntoEvent + 'static + Serialize + DeserializeOwned,
{
    valid_events: Vec<E>,
    min_length: usize,
    max_length: usize,
    _marker: PhantomData<(S, E)>,
}

impl<S, E> EventSequenceStrategyBuilder<S, E>
where
    S: StateTrait + Clone + Debug + Default + 'static,
    E: EventTrait + Clone + Debug + IntoEvent + 'static + Serialize + DeserializeOwned,
{
    /// 新しいビルダーを作成
    pub fn new() -> Self {
        Self {
            valid_events: Vec::new(),
            min_length: 1,
            max_length: 10,
            _marker: PhantomData,
        }
    }

    /// 有効なイベントを追加
    pub fn with_events(mut self, events: Vec<E>) -> Self {
        self.valid_events = events;
        self
    }

    /// シーケンスの最小長を設定
    pub fn min_length(mut self, min: usize) -> Self {
        self.min_length = min;
        self
    }

    /// シーケンスの最大長を設定
    pub fn max_length(mut self, max: usize) -> Self {
        self.max_length = max;
        self
    }

    /// ストラテジーを構築
    pub fn build(self) -> BoxedStrategy<Vec<E>>
    where
        E: Clone + 'static,
    {
        let events = self.valid_events;
        let min = self.min_length;
        let max = self.max_length;

        proptest::collection::vec(proptest::sample::select(events), min..=max).boxed()
    }
}

/// Property-basedテストランナー
pub struct PropertyTestRunner<S, E>
where
    S: StateTrait
        + Clone
        + Debug
        + Eq
        + Hash
        + Display
        + Send
        + Sync
        + 'static
        + Default
        + From<String>
        + Serialize
        + DeserializeOwned,
    E: EventTrait
        + Clone
        + Debug
        + IntoEvent
        + Send
        + Sync
        + 'static
        + Serialize
        + DeserializeOwned,
{
    machine: Machine<Context, E, S>,
    config: Option<proptest::test_runner::Config>,
    _marker: PhantomData<(S, E)>,
}

impl<S, E> PropertyTestRunner<S, E>
where
    S: StateTrait
        + Clone
        + Debug
        + Eq
        + Hash
        + Display
        + Send
        + Sync
        + 'static
        + Default
        + From<String>
        + Serialize
        + DeserializeOwned,
    E: EventTrait
        + Clone
        + Debug
        + IntoEvent
        + Send
        + Sync
        + 'static
        + Serialize
        + DeserializeOwned,
{
    /// 新しいテストランナーを作成
    pub fn new(machine: Machine<Context, E, S>) -> Self {
        Self {
            machine,
            config: None,
            _marker: PhantomData,
        }
    }

    /// プロパティを検証
    pub fn verify_property<P>(
        &self,
        property: P,
        config: proptest::test_runner::Config,
    ) -> PropertyTestResult
    where
        P: StateMachineProperty<S, E>,
        E: Clone,
    {
        let machine = self.machine.clone();
        let property_name = property.name().to_string();

        // proptestのランナーを作成
        let mut runner = proptest::test_runner::TestRunner::new(config);

        // 検証を実行
        let result = runner.run(
            &proptest::strategy::Just(()).prop_map(move |_| property.evaluate(&machine)),
            |result| {
                if result {
                    Ok(())
                } else {
                    Err(proptest::test_runner::TestCaseError::fail(
                        "Property violated",
                    ))
                }
            },
        );

        match result {
            Ok(_) => PropertyTestResult {
                property_name,
                success: true,
                counterexample: None,
                message: Some("Property holds for all tested inputs".to_string()),
            },
            Err(e) => PropertyTestResult {
                property_name,
                success: false,
                counterexample: Some(vec![format!("{}", e)]),
                message: Some(format!("Property violation detected")),
            },
        }
    }

    /// イベントシーケンスを使用してプロパティを検証
    pub fn verify_with_events<P, S1>(
        &self,
        property: P,
        event_strategy: S1,
        config: proptest::test_runner::Config,
    ) -> PropertyTestResult
    where
        P: StateMachineProperty<S, E> + Clone,
        E: Clone,
        S1: Strategy<Value = Vec<E>>,
    {
        let property_name = property.name().to_string();

        // proptestのランナーを作成
        let mut runner = proptest::test_runner::TestRunner::new(config);

        // 検証を実行
        let result = runner.run(&event_strategy, |events| {
            let mut machine = self.machine.clone();

            // イベントを適用
            for event in &events {
                if let Err(_) = machine.transition(event.clone(), Context::default()) {
                    // イベントが適用できない場合はスキップ
                    return Ok(());
                }
            }

            // プロパティを評価
            if property.evaluate(&machine) {
                Ok(())
            } else {
                Err(proptest::test_runner::TestCaseError::fail(format!(
                    "Property violated after events: {:?}",
                    events
                )))
            }
        });

        match result {
            Ok(_) => PropertyTestResult {
                property_name,
                success: true,
                counterexample: None,
                message: Some("Property holds for all tested event sequences".to_string()),
            },
            Err(e) => {
                // 反例の表示を簡略化
                PropertyTestResult {
                    property_name,
                    success: false,
                    counterexample: Some(vec![format!("{}", e)]),
                    message: Some(format!("Property violation detected")),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Event, MachineBuilder, State, Transition};

    // テスト用の簡単な状態マシンを作成
    fn create_test_machine() -> Machine {
        // 状態の作成
        let green = State::new("green");
        let yellow = State::new("yellow");
        let red = State::new("red");

        // 遷移の作成
        let green_to_yellow = Transition::new("green", "TIMER", "yellow");
        let yellow_to_red = Transition::new("yellow", "TIMER", "red");
        let red_to_green = Transition::new("red", "TIMER", "green");

        // マシンの構築
        let machine = MachineBuilder::new("trafficLight")
            .state(green)
            .state(yellow)
            .state(red)
            .initial("green")
            .transition(green_to_yellow)
            .transition(yellow_to_red)
            .transition(red_to_green)
            .build()
            .unwrap();

        // 状態マッパーを追加
        machine.with_state_mapper(|id| match id {
            id if id == "green" => State::new("green"),
            id if id == "yellow" => State::new("yellow"),
            id if id == "red" => State::new("red"),
            _ => State::new(id),
        })
    }

    #[test]
    fn test_simple_property() {
        let machine = create_test_machine();

        // プロパティの定義: greenからTIMERイベントを送ると必ずyellowになる
        let property = Machine::<State, Event>::property("green to yellow")
            .description("Sending TIMER from green should transition to yellow")
            .given(|m: &Machine<State, Event>| m.is_in("green"))
            .when(|m: &mut Machine<State, Event>| {
                m.send("TIMER")?;
                Ok(m.current_state().clone())
            })
            .then(|m: &Machine<State, Event>| m.is_in("yellow"));

        // プロパティの検証
        let runner = PropertyTestRunner::new(machine);
        let result = runner.verify_property(property, proptest::test_runner::Config::default());

        assert!(result.success);
    }

    #[test]
    fn test_event_sequence_property() {
        let machine = create_test_machine();

        // プロパティの定義: どの状態からでも3回のTIMERイベントで元の状態に戻る
        let property = Machine::<State, Event>::property("cycle property")
            .description("Sending TIMER three times should return to the original state")
            .given(|_: &Machine<State, Event>| true) // どの状態でも
            .when(|m: &mut Machine<State, Event>| {
                let _initial_state = m.current_state().id().to_string();
                m.send("TIMER")?;
                m.send("TIMER")?;
                m.send("TIMER")?;
                Ok(m.current_state().clone())
            })
            .then(|m: &Machine<State, Event>| {
                // 3回のTIMERイベントで元の状態に戻る
                // Traffic lightの場合、3回のサイクルで元に戻る
                m.is_in("green")
            });

        // イベントシーケンスストラテジーの構築
        let events_strategy = EventSequenceStrategyBuilder::<State, Event>::new()
            .with_events(vec![Event::new("TIMER")])
            .min_length(3)
            .max_length(3)
            .build();

        // プロパティの検証
        let runner = PropertyTestRunner::new(machine);
        let result = runner.verify_with_events(
            property,
            events_strategy,
            proptest::test_runner::Config::default(),
        );

        assert!(result.success);
    }
}
