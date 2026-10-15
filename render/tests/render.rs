use std::cell::RefCell;
use std::rc::Rc;

use crownplugin_render::{EventSink, Resolver, render};
use crownplugin_view::{ButtonVariant, Node, NodeId, Tree, UiEvent, parse_class};
use crowntest::{Role, TestApp, role, text};
use view::{AnyView, Cx, View, component};

#[derive(Default)]
struct Host {
    events: Rc<RefCell<Vec<UiEvent>>>,
}

impl Resolver for Host {
    fn events(&self) -> Option<EventSink> {
        let events = Rc::clone(&self.events);
        Some(Rc::new(move |event| events.borrow_mut().push(event)))
    }

    fn placeholder(&self, name: &str, _cx: &mut Cx) -> Option<AnyView> {
        (name == "lock.clock").then(|| widgets::text("12:00").boxed())
    }
}

fn tree() -> Tree {
    Tree {
        rev: 1,
        root: Node::VStack {
            style: parse_class("gap-2 p-4 bg-surface.raised").expect("valid"),
            children: vec![
                Node::HStack {
                    style: parse_class("gap-1 items-center").expect("valid"),
                    children: vec![
                        Node::Icon { style: parse_class("text-sm text-text.muted").expect("valid"), name: "cpu".into() },
                        Node::Text { style: parse_class("font-semibold").expect("valid"), text: "CPU 42%".into() },
                    ],
                },
                Node::Placeholder { name: "lock.clock".into() },
                Node::Button { id: NodeId(0), style: Vec::new(), label: "Refresh".into(), icon: Some("refresh-cw".into()), variant: ButtonVariant::Primary },
                Node::Toggle { id: NodeId(1), label: "Graph".into(), on: false },
            ],
        },
    }
}

fn app(host: Host) -> TestApp {
    let tree = tree();
    TestApp::new(component(move |cx| render(cx, &tree, &host)))
}

#[test]
fn text_placeholders_and_controls_render() {
    let mut app = app(Host::default());
    assert_eq!(app.text_of(text("CPU")), "CPU 42%");
    assert_eq!(app.text_of(text("12:")), "12:00");
    app.find(role(Role::Button).name("Refresh"));
    app.find(role(Role::Switch).name("Graph"));
}

#[test]
fn interactions_come_back_as_ui_events() {
    let host = Host::default();
    let events = Rc::clone(&host.events);
    let mut app = app(host);
    app.click(role(Role::Button).name("Refresh"));
    app.click(role(Role::Switch).name("Graph"));
    app.settle();
    assert_eq!(*events.borrow(), vec![UiEvent::Click(NodeId(0)), UiEvent::Toggle(NodeId(1), true)]);
}
