use crownplugin_view::{Node, NodeId, Tree, parse_class};

#[test]
fn trees_survive_postcard() {
    let tree = Tree {
        rev: 7,
        root: Node::HStack {
            style: parse_class("gap-1 items-center").expect("valid"),
            children: vec![
                Node::Icon { style: vec![], name: "cpu".into() },
                Node::Text { style: parse_class("text-sm font-semibold").expect("valid"), text: "42%".into() },
                Node::Slider { id: NodeId(3), value: 0.5, min: 0.0, max: 1.0, step: 0.1 },
            ],
        },
    };
    let bytes = postcard::to_stdvec(&tree).expect("encodes");
    assert_eq!(postcard::from_bytes::<Tree>(&bytes).expect("decodes"), tree);
}

#[test]
fn decoding_rechecks_tokens() {
    let style = crownplugin_view::Style::Bg(crownplugin_view::Token::new("surface.bg").expect("token"));
    let mut bytes = postcard::to_stdvec(&style).expect("encodes");
    let at = bytes.len() - "surface.bg".len();
    bytes[at..].copy_from_slice(b"surface.zz");
    assert!(postcard::from_bytes::<crownplugin_view::Style>(&bytes).is_err());
}
