#![cfg(feature = "base_component")]

use lunelle_aom::{Component, Element};
use lunelle_macros::{component, view};

#[component]
fn greeting(name: String) -> lunelle_aom::P {
    lunelle_aom::P::new().with_text(name)
}

#[test]
fn generated_view_builds_a_public_aom_tree() {
    let element = view! {
        <div id="app" class="shell">
            <button class="primary">"Save"</button>
        </div>
    };

    assert_eq!(element.tag_name(), "div");
    assert_eq!(element.element_id(), Some("app"));
    assert_eq!(element.classes(), ["shell"]);
    let child = &Element::children(&element)[0];
    assert_eq!(child.tag_name(), "button");
    assert_eq!(child.classes(), ["primary"]);
    assert_eq!(child.children()[0].tag_name(), "text");
    assert_eq!(child.children()[0].text(), Some("Save"));
}

#[test]
fn component_attribute_generates_a_public_aom_component() {
    let mut component = GreetingComponent::create(GreetingProps {
        name: "Ada".to_owned(),
    });
    assert_eq!(component.view().text(), Some("Ada"));
    component.update(GreetingProps {
        name: "Grace".to_owned(),
    });
    assert_eq!(component.view().text(), Some("Grace"));
}

#[derive(Default)]
struct CounterState {
    value: usize,
}

enum CounterEvent {
    Add(usize),
}

#[component(state = CounterState, event = CounterEvent)]
fn counter(initial: usize) -> impl lunelle_aom::IntoView {
    #[init]
    fn initialize(&mut self) {
        self.state.value = self.props.initial;
    }

    #[state]
    fn handle(&mut self, event: CounterEvent) {
        match event {
            CounterEvent::Add(value) => self.state.value += value,
        }
    }

    view! { <p>{self.state.value}</p> }
}

#[test]
fn stateful_component_initializes_and_updates_its_view() {
    let mut instance =
        lunelle_aom::ComponentInstance::<CounterComponent>::new(CounterProps { initial: 2 });
    assert_eq!(instance.element().children()[0].text(), Some("2"));
    instance.dispatch(CounterEvent::Add(3));
    assert_eq!(instance.element().children()[0].text(), Some("5"));
}

#[derive(Default)]
struct ActionState {
    value: usize,
}

enum ActionMessage {
    Reset,
}

#[component(state = ActionState, event = lunelle_aom::Event<ActionMessage>)]
fn action_counter() -> impl lunelle_aom::IntoView {
    #[state]
    fn handle(&mut self, event: lunelle_aom::Event<ActionMessage>) {
        match event {
            lunelle_aom::Event::Click(event)
                if event.target.as_deref() == Some("increment") =>
            {
                self.state.value += 1;
            }
            lunelle_aom::Event::Custom(ActionMessage::Reset) => self.state.value = 0,
            _ => {}
        }
    }

    view! {
        <div>
            <button id="increment">"Increment"</button>
            <p>{self.state.value}</p>
        </div>
    }
}

#[test]
fn action_updates_another_child_and_custom_messages_still_work() {
    use lunelle_aom::event::MouseButton;
    use lunelle_aom::{ComponentInstance, Event, UiAction};

    let mut instance = ComponentInstance::<ActionCounterComponent>::new(());
    instance.dispatch_action(UiAction::Click {
        target: Some("increment".into()),
        button: MouseButton::Left,
    });
    assert_eq!(
        instance.element().children()[1].children()[0].text(),
        Some("1")
    );
    instance.dispatch(Event::Custom(ActionMessage::Reset));
    assert_eq!(
        instance.element().children()[1].children()[0].text(),
        Some("0")
    );

    let input: Event<()> = Event::from_action(UiAction::Input {
        target: Some("field".into()),
        value: "hello".into(),
    });
    assert!(matches!(input, Event::Input(event) if event.value.as_deref() == Some("hello")));
}
