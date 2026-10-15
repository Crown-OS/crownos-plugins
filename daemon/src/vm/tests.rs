use crownplugin_model::Package;
use crownplugin_proto::Part;
use crownplugin_view::Node;

use super::*;

fn package(script: &str, capabilities: &str) -> (tempfile::TempDir, Package) {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(
        dir.path().join("plugin.ron"),
        format!(
            r#"Plugin(id: "dev.test.vm", name: "VM", version: "1.0.0",
                capabilities: [{capabilities}],
                contributes: (bar: [(id: "w")], lock: [(id: "s")]),
                limits: (memory_mb: 8, call_budget_ms: 50))"#
        ),
    )
    .expect("manifest");
    std::fs::write(dir.path().join("main.luau"), script).expect("script");
    let package = Package::load(dir.path()).expect("loads");
    (dir, package)
}

fn start(script: &str, capabilities: &str, denied: &[&str]) -> (tempfile::TempDir, Vm, Outcome) {
    let (dir, package) = package(script, capabilities);
    let grants = Grants {
        declared: package.manifest.capabilities(),
        denied: denied.iter().map(|key| (*key).to_owned()).collect(),
    };
    let (vm, outcome) = Vm::start(&package, &grants, Seed::default()).expect("vm starts");
    (dir, vm, outcome)
}

fn global(vm: &Vm, name: &str) -> mlua::Value {
    vm.lua().globals().get(name).expect("readable")
}

#[test]
fn a_runaway_loop_is_interrupted() {
    let (_dir, mut vm, outcome) = start("return { actions = { spin = function() while true do end end } }", "", &[]);
    assert!(matches!(outcome, Outcome::Ok));
    assert!(matches!(vm.action("spin"), Outcome::Failed(message) if message.contains("time budget")));
}

#[test]
fn a_memory_bomb_fails_without_taking_the_daemon_down() {
    let script = r#"return { actions = { grow = function()
        local t = {}
        for i = 1, 1e9 do t[i] = string.rep("x", 1024) .. i end
    end } }"#;
    let (_dir, mut vm, _) = start(script, "", &[]);
    assert!(matches!(vm.action("grow"), Outcome::Failed(_)));
}

#[test]
fn three_failures_quarantine_the_vm() {
    let (_dir, mut vm, _) = start("return { actions = { boom = function() error('no') end } }", "", &[]);
    assert!(matches!(vm.action("boom"), Outcome::Failed(_)));
    assert!(matches!(vm.action("boom"), Outcome::Failed(_)));
    assert!(matches!(vm.action("boom"), Outcome::Quarantine(_)));
}

#[test]
fn denied_and_undeclared_apis_do_not_exist() {
    let (_dir, vm, _) = start("", r#"Network(hosts: ["example.com"]), Notify"#, &["network"]);
    assert!(global(&vm, "http").is_nil(), "denied");
    assert!(!global(&vm, "notify").is_nil(), "declared and undecided");
    assert!(global(&vm, "exec").is_nil(), "never declared");
    assert!(global(&vm, "io").is_nil());
}

#[test]
fn the_sandbox_cannot_be_escaped_through_globals() {
    let (_dir, vm, outcome) = start("ui.render = nil", "", &[]);
    assert!(matches!(outcome, Outcome::Failed(_)), "builtin tables are read-only");
    let loaded: mlua::Value = vm.lua().load("return os.execute").eval().expect("evaluates");
    assert!(loaded.is_nil());
}

#[test]
fn render_queues_a_tree_and_keeps_its_handlers() {
    let script = r#"
        local clicks = 0
        ui.render("w",
            ui.hstack({ class = "gap-1 items-center" }, { ui.icon("cpu"), ui.text("42%", "text-sm") }),
            ui.vstack({}, { ui.button({ label = "Go", on_click = function() clicks += 1; log.info("clicked " .. clicks) end }) }))
        return {}
    "#;
    let (_dir, mut vm, outcome) = start(script, "", &[]);
    assert!(matches!(outcome, Outcome::Ok), "{outcome:?}");
    let commands = vm.take_commands();
    let trees: Vec<&Option<Node>> = commands
        .iter()
        .filter_map(|command| match command {
            Command::Render { tree, .. } => Some(tree),
            _ => None,
        })
        .collect();
    assert_eq!(trees.len(), 2);
    assert!(matches!(trees[0], Some(Node::HStack { children, .. }) if children.len() == 2));

    let click = crownplugin_view::UiEvent::Click(crownplugin_view::NodeId(0));
    assert!(matches!(vm.ui_event("w", Part::Popup, &click), Outcome::Ok));
    assert!(vm
        .take_commands()
        .iter()
        .any(|command| matches!(command, Command::Log(_, message) if message == "clicked 1")));
}

#[test]
fn literal_colors_and_undeclared_widgets_are_script_errors() {
    let (_dir, _vm, outcome) = start(r#"ui.render("w", ui.text("x", "bg-[#fff]"))"#, "", &[]);
    assert!(matches!(outcome, Outcome::Failed(message) if message.contains("literal colors")));
    let (_dir, _vm, outcome) = start(r#"ui.render("nope", ui.text("x"))"#, "", &[]);
    assert!(matches!(outcome, Outcome::Failed(message) if message.contains("not declared")));
}

#[test]
fn lock_slots_never_carry_inputs() {
    let script = r#"ui.render("s", ui.vstack({}, { ui.text("hi"), ui.button({ label = "x" }) }))"#;
    let (_dir, vm, _) = start(script, "", &[]);
    let commands = vm.take_commands();
    let Some(Command::Render { tree: Some(tree), .. }) = commands.first() else {
        panic!("expected a render, got {commands:?}");
    };
    assert!(!tree.any(&Node::is_interactive));
}

#[test]
fn timers_register_their_callbacks() {
    let (_dir, mut vm, _) = start("timer.every(10, function() log.info('tick') end)", "", &[]);
    let commands = vm.take_commands();
    let Some(Command::Timer { id, every, repeat: true }) = commands.first() else {
        panic!("expected a timer, got {commands:?}");
    };
    assert_eq!(*every, std::time::Duration::from_millis(250), "clamped to the minimum");
    assert!(matches!(vm.fire_timer(*id), Some(Outcome::Ok)));
}

#[test]
fn every_global_is_declared_in_the_type_definitions() {
    const TYPES: &str = include_str!("../../../types/crown.d.luau");
    let everything = r#"Network(hosts: ["example.com"]), Exec(programs: ["true"]), FsRead(Music),
        FsWrite(Music), Notify, CompositorActions, WindowFeed, ThemeVisual"#;
    let (_dir, vm, _) = start("", everything, &[]);
    let globals = [
        "ui", "plugin", "log", "timer", "settings", "theme", "feed", "compositor", "config",
        "wallpaper", "http", "exec", "fs", "notify",
    ];
    for name in globals {
        assert!(TYPES.contains(&format!("declare {name}:")), "`{name}` is not declared");
        let table: mlua::Table = vm
            .lua()
            .globals()
            .get(name)
            .unwrap_or_else(|error| panic!("`{name}` is missing: {error}"));
        for pair in table.pairs::<String, mlua::Value>() {
            let (key, _) = pair.expect("string keys");
            assert!(
                TYPES.contains(&format!("\t{key}:")),
                "`{name}.{key}` exists at runtime but not in crown.d.luau"
            );
        }
    }
}
