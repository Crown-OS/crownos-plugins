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
