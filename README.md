# crownos-plugins

Themes and plugins for CrownOS.

- A **theme** restyles the whole desktop: theme tokens, blur and other appearance settings, the wallpaper, the lock screen, the bar, cursors, icons, fonts, sounds and GTK/Qt colors.
- A **plugin** adds behaviour: bar widgets with popups, lock-screen slots, a settings page, keybindings, window rules and reactions to the desktop.

Both are a folder holding a RON manifest and sandboxed Luau, so an AI can write one in a single pass.

| Crate | What it is |
|---|---|
| `model` → `crownplugin-model` | Manifests, capabilities and their levels, settings fields, package paths, `.crownpkg` archives, validation |
| `view` → `crownplugin-view` | The UI-as-data tree: crownui kit node names, Tailwind classes that allow only token colors, and UI events |
| `proto` → `crownplugin-proto` | The `crownplugind` crownos-ipc protocol |
| `render` → `crownplugin-render` | Tree → crownui views, used by the bar, Settings and the compositor's lock UI |
| `daemon` → `crownplugind` | Runs everything: one Luau VM per entity, the journal, permissions, the compositor link |
| `cli` → `crownplugin` | `new`, `validate`, `pack`, `install`, `dev`, `list`, `enable`, `theme`, `logs`, `set`, `grant`, `skill` |
| `sysassets` → `crownplugin-sysassets` | The pkexec helper that installs a theme's cursors and lock fonts system-wide |

`types/crown.d.luau` holds the typed script API. `skill/SKILL.md` is the authoring guide for coding agents (`crownplugin skill` installs both). `examples/` has a theme and three plugins.

## How it works

```
~/.local/share/crownos/{themes,plugins}/<id>/        (+ /usr/share/crownos/…)
                │
         crownplugind ── journal + write-through ──▶ ~/.config/crownos/*.ron ──▶ every app
          │  one sandboxed Luau VM per running entity
          ├── crownos-ipc "crownplugind" ◀── crownbar · crownos-settings · crownplugin
          ├── crownos_plugin_host_v1 (Wayland) ◀──▶ crownpositor: actions, window events,
          │                                         plugin keybindings, lock-screen slots and assets
          └── org.freedesktop.impl.portal.Access ──▶ xdg-desktop-portal-crownos (permission prompts)
```

- **Write-through with a journal.**
  - Turning an entity on writes its changes straight into the real config files. Before each write, the old value is recorded in `$XDG_STATE_HOME/crownos/plugins/journal/<id>.ron`.
  - Turning it off puts back only the values that still hold what the entity wrote, so anything the user changed in the meantime survives.
  - When two entities write the same key, the records are spliced so that neither one can resurrect a stale value.
  - A crash between journal and config is undone at the next start.
- **UI is data.**
  - Scripts build trees with `ui.*`. Colors are theme tokens; literal colors are rejected.
  - The bar, Settings and the lock screen render the trees with their own crownui widgets, so plugins always look native.
  - A tree is re-sent only when it changes, at most 30 times a second.
- **Three permission levels.**
  - Safe capabilities just work.
  - Sensitive ones are asked about on first use.
  - Dangerous ones are asked with a warning.
  - Decisions live in `plugins.ron`, and Settings can revoke them. A denied API does not exist in the script.
- **Isolation.** Each VM has a memory limit and a time budget per call. Three failures within a minute quarantine the entity. HTTP is limited to the manifest's hosts, programs to its list, and files to the user folders it declared.

## Install

```sh
sudo ./install.sh
systemctl --user enable --now crownplugind.socket
crownplugin skill        # authoring skill for Claude Code and other agents
```

## Develop

```sh
cargo test                          # model, view, proto, daemon (incl. e2e over IPC), cli
cargo test -p crownplugin-render    # crownui rendering, headless
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

`crownplugin dev <dir>` runs a package in place, reloads it on every save and streams its log.
