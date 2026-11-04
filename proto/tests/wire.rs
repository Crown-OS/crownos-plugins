use crownplugin_model::{Field, SettingValue, SettingsSection};
use crownplugin_proto::{EntityInfo, EntityStatus, Kind};

#[test]
fn entity_info_with_settings_survives_postcard() {
    let info = EntityInfo {
        id: "dev.test.cpu".into(),
        kind: Kind::Plugin,
        name: "CPU".into(),
        version: "1.0.0".into(),
        author: String::new(),
        description: String::new(),
        dir: "/tmp/x".into(),
        system: false,
        active: true,
        dev: false,
        status: EntityStatus::Running,
        capabilities: vec![],
        settings: Some(SettingsSection {
            title: "CPU".into(),
            icon: "cpu".into(),
            description: String::new(),
            fields: vec![Field::Toggle {
                key: "graph".into(),
                label: "Graph".into(),
                description: String::new(),
                default: true,
            }],
        }),
    };
    let bytes = postcard::to_stdvec(&info).expect("encodes");
    assert_eq!(postcard::from_bytes::<EntityInfo>(&bytes).expect("decodes"), info);

    let value = SettingValue::Number(2.5);
    let bytes = postcard::to_stdvec(&value).expect("encodes");
    assert_eq!(postcard::from_bytes::<SettingValue>(&bytes).expect("decodes"), value);
}

#[test]
fn launcher_snapshots_and_search_results_survive_postcard() {
    use crownplugin_proto::{
        LauncherSize, LauncherSnapshot, LauncherSurface, Node, SearchHit, SearchResults, Tree,
    };

    let snapshot = LauncherSnapshot {
        revision: 3,
        tiles: vec![LauncherSurface {
            plugin: "dev.test.quotes".into(),
            widget: "quote".into(),
            size: LauncherSize::Wide,
            tree: Tree { rev: 1, root: Node::Spacer },
        }],
    };
    let bytes = postcard::to_stdvec(&snapshot).expect("encodes");
    assert_eq!(postcard::from_bytes::<LauncherSnapshot>(&bytes).expect("decodes"), snapshot);
    assert_eq!(snapshot.tiles[0].entry(), "plugin:dev.test.quotes/quote");

    let results = SearchResults {
        plugin: "dev.test.calc".into(),
        provider: "calc".into(),
        name: "Calculator".into(),
        query: "2+2".into(),
        hits: vec![SearchHit {
            id: "4".into(),
            title: "4".into(),
            subtitle: "2+2".into(),
            icon: Some("calculator".into()),
        }],
    };
    let bytes = postcard::to_stdvec(&results).expect("encodes");
    assert_eq!(postcard::from_bytes::<SearchResults>(&bytes).expect("decodes"), results);
}
