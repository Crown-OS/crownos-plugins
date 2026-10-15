---
name: crownos-plugins
description: Create, edit and debug CrownOS themes and plugins — theme.ron/plugin.ron manifests, Luau scripts, bar widgets, lock-screen slots, settings pages, theme tokens and capabilities. Use whenever the user wants to restyle CrownOS (colors, blur, wallpaper, lock screen, bar) or add behaviour to it (bar widgets, keybindings, window rules), or mentions crownplugin, crownplugind, a .crownpkg, theme.ron or plugin.ron.
---

# Authoring CrownOS themes and plugins

A **theme** is one folder with `theme.ron`: the whole look of the desktop. Exactly one theme is active.
A **plugin** is one folder with `plugin.ron` plus a Luau script: widgets, settings, keybindings, behaviour. Any number can be enabled.

Both are data plus sandboxed Luau, run by `crownplugind`. No compiling.

## Workflow (always follow it)

1. `crownplugin new theme|plugin <reverse.dns.id> --name "Pretty Name"` creates a working template. Start from it; don't write from scratch.
2. Edit the files.
3. `crownplugin validate <dir>` until it prints `ok`.
   - Every error is `path.in.manifest: message` or `file.luau:line: message`.
   - Fix exactly what it says, then run it again.
4. `crownplugin dev <dir>` runs the folder in place, reloads it on every save and streams its log. Runtime errors appear there with a line number.
5. When it's done: `crownplugin pack <dir>` writes a `.crownpkg` to share. `crownplugin install <dir|file|https-git-url> --enable` installs it.

`crownplugin skill --types` prints the full Luau API (`crown.d.luau`). Read it before writing a script; it is the authority on every function.

## Rules that keep packages valid

- **Ids** are reverse-DNS, lowercase, `a-z 0-9 . - _`, for example `dev.alice.cpu-graph`. Widget, slot and action ids use `a-z 0-9 -`.
- **Versions** are semver: `"1.0.0"`.
- **Manifests are RON.**
  - Optional fields may be written bare: `wallpaper: (dark: "night.jpg")`.
  - Enums are bare names: `Blue`, `Large`, `Right`.
  - Strings are double-quoted.
- **Colors are theme tokens only.**
  - Classes use `bg-<token>`, `text-<token>`, `border-<token>`.
  - Literal colors (`bg-[#fff]`, `text-red-500`) are rejected.
  - Themes set token values as hex: `"#rrggbb"` or `"#rrggbbaa"`.
- **Paths** in a manifest are relative to the package folder and may not leave it.
- **Config values** in `config:` use the key's real RON type: `"blur_passes": 4`, `"accent": Blue`, `"floating": true`, `"wallpaper": "/abs/path"`.

## Theme tokens

- **Kit slots:** `surface.{bg, raised, sunken, border, hairline, hover, shadow}`, `text.{primary, body, muted, disabled, on_accent, icon, icon_muted}`, `control.{track, knob, knob_shadow}`, `status.{success, warning, danger, neutral, danger_bg, on_danger}`, `popover.{bg, border, text, muted_text, hover_bg, trigger_bg}`, `radio.{unselected_bg, unselected_border, hole, text}`, `menu.{bg, border, text, shortcut_text, disabled_text, header_text, separator, selected_bg, selected_text}`, `sidebar.{bg, separator, text, selected_text, subitem_text, brand_text, icon, selected_icon, hover_bg, selected_bg, selected_border}`, `toggle_off.{start, end}`, `accent.{start, end}`.
- **Window frames:** `frame.{tint_top, tint_bottom, highlight, control_fill, control_glyph, border, label}`.
- **Bar:** `bar.{fg, fg_hover}` (text and icons drawn directly on the bar).

## theme.ron

```ron
Theme(
    id: "dev.alice.midnight", name: "Midnight", version: "1.0.0",
    description: "Deep blue glass", license: "MIT",
    tokens: (
        dark:  {"surface.bg": "#0b0d12", "surface.raised": "#141824", "text.primary": "#eef0f6", "frame.tint_top": "#1a2030e0"},
        light: {"surface.bg": "#f4f6fb"},
        accent: Some((start: "#7aa2ff", end: "#4c6fff")),
        radii: Some((sm: 6.0, md: 10.0, lg: 16.0, xl: 24.0)),
        font: Some((family: "Inter", mono: "JetBrains Mono", scale: 1.0)),
    ),
    config: {
        "appearance": {"dark_mode": true, "blur": true, "blur_passes": 4, "blur_size": 2.0, "border_radius": 12, "gaps_inner": 10, "transparency": 0.15},
        "bar": {"floating": true, "margin": 8, "radius": 14, "position": Top},
    },
    wallpaper: (dark: "wallpapers/night.jpg", light: "wallpapers/day.jpg"),
    lock: (background: Image("wallpapers/lock.jpg"), clock_style: Large, dim: 0.35, blur: 0.8, transition: Zoom),
    assets: (icons: "icons/Midnight", cursors: "cursors/Midnight", fonts: ["fonts"], sounds: "sounds/midnight", system: "system", gtk: true, qt: true),
    script: Some("theme.luau"),
)
```

- `config` may write `appearance`, `theme`, `bar` and `lock` freely. Any other section needs a capability (below).
- **`appearance` keys:**
  - `dark_mode`, `accent` (`Purple|Blue|Green|Orange|Pink`), `transparency` (0–1), `wallpaper`, `bar_height`;
  - `gaps_inner`, `gaps_outer`, `border_width`, `border_radius`, `animations` (`None|Snappy|Standard|Smooth`), `titlebar_height`;
  - `blur`, `blur_passes`, `blur_size`, `blur_noise`, `cursor_theme`, `icon_theme`.
- **`bar` keys:**
  - `height`, `position` (`Top|Bottom`), `floating`, `margin`, `radius`;
  - `left`, `center`, `right`: lists of widget names (built-ins: `clock`, `layout`, `weather`, `caffeine`, `volume`, `brightness`, `bluetooth`, `wifi`, `battery`, `notifications`).
- **`lock`:**
  - `clock_style` (`Classic|Large|Minimal|Stacked`), `time_format` and `date_format` (strftime);
  - `font`, a system font family; ship it under `assets.system/fonts`;
  - `layout` (`Center|Left|Bottom`), `background` (`Desktop | Image("rel/path") | Token("surface.bg")`), `dim`, `blur`, `transition` (`Rise|Fade|Zoom|None`).
- **`gtk` / `qt`** generate matching colors for third-party apps from the tokens.
- **`script`** (optional) is Luau with the same API as plugins, for dynamic looks such as a wallpaper that follows the time of day (`wallpaper.set`). It can also lay out the lock screen with `ui.lock_layout` and placeholders.

## plugin.ron

```ron
Plugin(
    id: "dev.alice.cpu-graph", name: "CPU Graph", version: "1.0.0",
    entry: "main.luau",
    capabilities: [Feed(System)],
    contributes: (
        bar: [(id: "graph", slot: Right, after: Some("volume"))],
        lock: [(id: "load", slot: BelowClock)],
        settings: (title: "CPU Graph", icon: "cpu", fields: [
            Slider(key: "interval", label: "Refresh every", min: 1.0, max: 10.0, step: 1.0, unit: "s", default: 2.0),
            Toggle(key: "label", label: "Show percentage", default: true),
            Select(key: "style", label: "Style", options: [(value: "bars", label: "Bars"), (value: "line", label: "Line")], default: "bars"),
            ColorToken(key: "color", label: "Color", default: "accent.end"),
            Text(key: "city", label: "City", default: "Berlin"),
        ]),
        keybinds: [(keys: "Super+Shift+G", action: "toggle")],
        window_rules: [(app_id: Some("pavucontrol"), floating: Some(true))],
        config: {},
    ),
    limits: (memory_mb: 16, call_budget_ms: 20),
)
```

- **`bar` widgets:**
  - `slot` is `Left`, `Center` or `Right`;
  - `after` places the widget after a named entry;
  - `replaces: Some("clock")` takes a built-in's place.
- **`lock` slots:**
  - positions are `Clock` (replaces the clock), `AboveClock`, `BelowClock` and `Bottom`;
  - they are display-only, so buttons are stripped.
- **`settings`** appear as the plugin's own page in Settings; values are read with `settings.get`.
- **`keybinds`** call `actions.<action>` in the module the script returns. A chord the user already uses is skipped.

## Capabilities

| Level | Capabilities | What happens |
|---|---|---|
| Safe | `Feed(Time\|System\|Workspaces\|Power)`, widgets, slots, a settings page, own settings, `ThemeVisual` | Work immediately; no need to list them. |
| Sensitive | `Network(hosts: ["api.x.com"])`, `FsRead(Music)`, `Notify`, `Config(section: "input")`, `WindowFeed`, `WindowRules`, `CompositorActions` | The user is asked on first use. |
| Dangerous | `Exec(programs: ["playerctl"])`, `FsWrite(Documents)`, `Keybinds`, `PowerConfig`, `DisplayConfig`, `StartupConfig` | Asked with a warning on first use. |

- Ask for as little as possible: most plugins need only safe capabilities.
- A denied or undeclared API is simply `nil` in the script. Guard optional features with `if http then ... end`.
- User folders are `Home`, `Documents`, `Downloads`, `Music`, `Pictures` and `Videos`.

## Luau script

```lua
local function draw()
    local s = feed.system()
    ui.render("graph",
        ui.hstack({ class = "gap-1 items-center" }, {
            ui.icon("cpu", "text-bar.fg"),
            ui.text(string.format("%d%%", s.cpu * 100), "text-sm font-semibold text-bar.fg"),
        }),
        ui.card({ class = "p-3 gap-2 w-[240]" }, {
            ui.text("Processor", "text-sm font-semibold text-text.primary"),
            ui.progress(s.cpu),
            ui.button({ label = "Open monitor", variant = "secondary", on_click = function()
                if compositor.run then compositor.run("focus-workspace 3") end
            end }),
        }))
end

local tick = timer.every(settings.get("interval") * 1000, draw)
settings.on_change(function() timer.cancel(tick); tick = timer.every(settings.get("interval") * 1000, draw) end)
draw()

return {
    actions = { toggle = function() log.info("toggled") end },
    on_popup = function(widget, open) if open then draw() end end,
}
```

- **`ui.render(id, pill, popup?)`** draws a bar widget (pill plus optional popup panel) or a lock slot. Re-rendering an identical tree costs nothing, so redraw from timers freely.
- **Nodes:** `vstack`, `hstack`, `zstack`, `card`, `text`, `icon` (Lucide names), `image` (package path), `spacer`, `divider`, `badge`, `progress`, `button`, `toggle`, `slider`, `select`.
- **Classes:**
  - spacing (`p-2`=8px, `px-[10]`, `gap-1`), size (`w-full`, `h-8`, `w-[200]`, `min-w-…`, `max-w-…`), `rounded-sm|md|lg|xl|full`;
  - type (`text-xs…7xl`, `font-medium|semibold|bold|mono`, `text-center`, `truncate`), layout (`items-*`, `justify-*`, `flex-1`), `opacity-50`, `border`, `hidden`.
- **Async APIs** (`http`, `exec`, `fs`) take a callback `function(result, err)`. They never block, and the first use may wait for the user's permission.
- **Each callback has a time budget** (`limits.call_budget_ms`). Loops that never yield are stopped; three failures in a minute stop the plugin until it is re-enabled.
- **Timers:** `timer.every(ms, fn)` (minimum 250 ms), `timer.after`, `timer.cancel`.
- **`require("lib/util")`** loads `lib/util.luau` from the package.

## Debugging

- Run `crownplugin logs <id> -f`, or `crownplugin dev <dir>`.
- `print(...)` and `log.info/warn/error` go to the log.
- `crownplugin list` shows the status (`Running`, `Error(..)`, `Quarantined(..)`).
- Each status maps to a cause:
  - **Error at enable:** usually a config value of the wrong type. Re-run validate.
  - **Quarantined:** the script failed three times. Read the log, fix it, and save; dev mode reloads it.
  - **Widget missing:** the id passed to `ui.render` must be declared in `contributes.bar`, and the bar lists it as `plugin:<id>/<widget>`.
- `crownplugin grant <id> <capability> allow|deny|ask` changes a decision. `crownplugin set <id> <key> <value>` changes a setting.
