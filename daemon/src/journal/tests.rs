use crownos_config::schema::{Appearance, Bar};

use super::*;
use crate::util::testenv::TestEnv;

fn set(section: &str, key: &str, value: &str) -> Change {
    Change::Set { section: section.into(), key: key.into(), value: value.into() }
}

fn no_system(_: bool) -> anyhow::Result<()> {
    Ok(())
}

fn enable(entity: &str, changes: Vec<Change>) -> Journal {
    let mut journal = Journal::new(entity);
    apply(&mut journal, changes, &mut no_system).expect("applies");
    journal
}

fn disable(entity: &str) {
    let mut journal = Journal::load(entity).expect("journal on disk");
    let mut others: Vec<Journal> = Journal::load_all().into_iter().filter(|other| other.entity != entity).collect();
    restore(&mut journal, &mut others, &mut no_system).expect("restores");
}

fn appearance() -> Appearance {
    crownos_config::load_strict(Appearance::SECTION).expect("parses")
}

#[test]
fn disabling_restores_the_files_byte_for_byte() {
    let env = TestEnv::new();
    crownos_config::save(Appearance::SECTION, &Appearance::default()).expect("save");
    crownos_config::save(Bar::SECTION, &Bar::default()).expect("save");
    let (before_appearance, before_bar) = (env.config("appearance"), env.config("bar"));

    enable(
        "crown.midnight",
        vec![
            set("appearance", "blur_passes", "5"),
            set("appearance", "accent", "Blue"),
            Change::MapSet { section: "theme".into(), key: "dark".into(), entry: "surface.bg".into(), value: r##""#101010""##.into() },
            Change::Insert { section: "bar".into(), key: "right".into(), item: r#""plugin:dev.a.cpu/graph""#.into(), after: Some(r#""volume""#.into()) },
        ],
    );
    let bar: Bar = crownos_config::load_strict(Bar::SECTION).expect("parses");
    let volume = bar.right.iter().position(|item| item == "volume").expect("volume");
    assert_eq!(bar.right[volume + 1], "plugin:dev.a.cpu/graph");
    assert_eq!(appearance().blur_passes, 5);

    disable("crown.midnight");
    assert_eq!(env.config("appearance"), before_appearance);
    assert_eq!(env.config("bar"), before_bar);
    let theme: crownos_config::Theme = crownos_config::load_strict("theme").expect("parses");
    assert!(theme.dark.is_empty());
    assert!(Journal::load("crown.midnight").is_none());
}

#[test]
fn a_value_the_user_changed_afterwards_is_kept() {
    let _env = TestEnv::new();
    enable("crown.a", vec![set("appearance", "blur_passes", "5")]);
    crownos_config::write_keys("appearance", [("blur_passes", "7")]).expect("user edit");
    disable("crown.a");
    assert_eq!(appearance().blur_passes, 7);
}

#[test]
fn stacked_writers_splice_so_no_stale_value_comes_back() {
    let _env = TestEnv::new();
    let original = Appearance::default().blur_passes;
    enable("crown.theme", vec![set("appearance", "blur_passes", "4")]);
    enable("dev.plugin", vec![set("appearance", "blur_passes", "6")]);

    disable("crown.theme");
    assert_eq!(appearance().blur_passes, 6, "the plugin's value stays while it is on");

    disable("dev.plugin");
    assert_eq!(appearance().blur_passes, original, "the user's value comes back, not the theme's");
}

#[test]
fn a_crash_before_the_config_write_restores_cleanly() {
    let _env = TestEnv::new();
    let journal = Journal {
        entity: "crown.crashed".into(),
        phase: Phase::Applying,
        ops: vec![Op::Set { section: "appearance".into(), key: "blur_passes".into(), before: "3".into(), after: "9".into() }],
    };
    journal.save().expect("save");
    crownos_config::write_keys("appearance", [("blur_passes", "3")]).expect("seed");
    disable("crown.crashed");
    assert_eq!(appearance().blur_passes, 3);
}

#[test]
fn repeated_sets_keep_the_first_before() {
    let _env = TestEnv::new();
    let mut journal = enable("crown.dynamic", vec![set("appearance", "blur_passes", "4")]);
    apply(&mut journal, vec![set("appearance", "blur_passes", "8")], &mut no_system).expect("applies");
    assert_eq!(journal.ops.len(), 1);
    disable("crown.dynamic");
    assert_eq!(appearance().blur_passes, Appearance::default().blur_passes);
}

#[test]
fn files_are_backed_up_and_put_back() {
    let env = TestEnv::new();
    let path = env.root.path().join("gtk.css");
    std::fs::write(&path, "user css").expect("seed");
    enable("crown.gtk", vec![Change::File { path: path.clone(), contents: b"theme css".to_vec() }]);
    assert_eq!(std::fs::read_to_string(&path).expect("read"), "theme css");
    disable("crown.gtk");
    assert_eq!(std::fs::read_to_string(&path).expect("read"), "user css");
}
