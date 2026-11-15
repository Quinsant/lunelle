use lunelle::{
    Application,
    aom::Event,
    component,
    core::{ResourceKind, ResourceTable},
    view,
};

#[derive(Default)]
pub struct CounterState {
    value: i32,
}

#[component(state = CounterState, event = Event<()>)]
pub fn Counter() -> impl IntoView {
    #[state]
    fn update(&mut self, event: Event<()>) {
        match event {
            Event::Click(event) if event.target.as_deref() == Some("increment") => {
                self.state.value += 1;
            }
            Event::Click(event) if event.target.as_deref() == Some("decrement") => {
                self.state.value -= 1;
            }
            _ => {}
        }
    }

    view! {
        <div class="counter-shell">
            <div class="counter-panel">
                <p class="eyebrow">"LUNELLE"</p>
                <p class="title">"Counter"</p>
                <p class="description">"A stateful component rendered by the native AOM runtime."</p>
                <div class="value-panel">
                    <p class="value-label">"Current value"</p>
                    <p class="value">{self.state.value}</p>
                </div>
                <div class="actions">
                    <button id="decrement" class="counter-button secondary">"- 1"</button>
                    <button id="increment" class="counter-button primary">"+ 1"</button>
                </div>
                <p class="status">"Click events, component state, CSS styles, diffing, and native rendering are active."</p>
            </div>
        </div>
    }
}

fn main() {
    let styles = r#"
        .counter-shell {
            display: flex;
            align-items: center;
            justify-content: center;
            min-height: 100vh;
            padding: 32px;
            background: linear-gradient(315deg, #195DB4 0%, #3FDCBF 100%);
        }
        .counter-panel {
            display: flex;
            flex-direction: column;
            gap: 16px;
            width: 420px;
            padding: 32px;
            text-align: center;
            border: 1px solid #cbd5e1;
            border-radius: 12px;
            background-color: #ffffff;
            box-shadow: 0 35px 60px -15px rgba(0, 0, 0, 0.25);
        }
        .eyebrow, .title, .description, .value-label, .value, .status {
            width: 100%;
            color: #64748b;
            font-size: 12px;
        }
        .title {
            color: #172033;
            font-size: 32px;
            font-weight: 700;
        }
        .description {
            color: #475569;
            font-size: 15px;
        }
        .value-panel {
            display: flex;
            flex-direction: column;
            align-items: center;
            gap: 8px;
            padding: 24px;
            border-radius: 8px;
            background-color: #f1f5f9;
        }
        .value {
            color: #195DB4;
            font-size: 56px;
            font-weight: 700;
        }
        .actions {
            display: flex;
            gap: 12px;
        }
        .counter-button {
            flex: 1;
            padding: 12px 20px;
            text-align: center;
            line-height: 40px;
            border-radius: 6px;
            font-size: 16px;
            font-weight: 700;
        }
        .primary {
            color: #ffffff;
            background-color: #195DB4;
        }
        .secondary {
            color: #ffffff;
            background-color: #3FDCBF;
        }
        .counter-button:hover {
            opacity: 0.8;
        }
        .counter-button:active {
            opacity: 0.6;
        }
        .status {
            width: 100%;
            min-height: 32px;
        }
    "#;
    let mut resources = ResourceTable::new();
    resources.add("counter.css", ResourceKind::Css, styles.as_bytes().to_vec());

    Application::builder()
        .resource(resources)
        .build()
        .run(|context| {
            context
                .spawn_window::<CounterComponent>(
                    (),
                    lunelle::core::WindowOptions {
                        title: "Lunelle Counter".to_owned(),
                        ..Default::default()
                    },
                )
                .expect("spawn counter window");
        })
        .expect("run counter application");
}
