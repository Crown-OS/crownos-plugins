--!strict
-- The globals a CrownOS theme or plugin script sees, as Luau type
-- declarations. Groups marked (capability) exist only when the manifest
-- declares that capability and the user has not denied it; test with
-- `if http then ... end` when a plugin should degrade gracefully.

export type Class = string -- Tailwind-style: "px-2 gap-1 items-center bg-surface.raised text-sm"

export type Node = {
	kind: string,
	class: Class?,
	[string]: any,
}

export type Tone = "neutral" | "accent" | "success" | "warning" | "danger"
export type ButtonVariant = "primary" | "secondary" | "ghost" | "danger"

declare ui: {
	vstack: (props: { class: Class?, [number]: Node }?, children: { Node }?) -> Node,
	hstack: (props: { class: Class?, [number]: Node }?, children: { Node }?) -> Node,
	zstack: (props: { class: Class?, [number]: Node }?, children: { Node }?) -> Node,
	card: (props: { class: Class?, [number]: Node }?, children: { Node }?) -> Node,
	text: (text: any, class: Class?) -> Node,
	-- A Lucide icon name, e.g. "cpu", "battery-charging", "music".
	icon: (name: string, class: Class?) -> Node,
	-- A path inside the package.
	image: (src: string, class: Class?) -> Node,
	spacer: () -> Node,
	divider: () -> Node,
	badge: (text: any, tone: Tone?) -> Node,
	-- 0..1
	progress: (value: number, class: Class?) -> Node,
	button: (props: {
		label: string,
		icon: string?,
		variant: ButtonVariant?,
		class: Class?,
		on_click: (() -> ())?,
	}) -> Node,
	toggle: (props: { label: string, on: boolean, on_change: ((on: boolean) -> ())? }) -> Node,
	slider: (props: {
		value: number,
		min: number?,
		max: number?,
		step: number?,
		on_change: ((value: number) -> ())?,
	}) -> Node,
	-- `selected` and the callback's index are 1-based.
	select: (props: { options: { string }, selected: number?, on_change: ((index: number) -> ())? }) -> Node,
	-- Only inside ui.lock_layout: "lock.clock", "lock.date", "lock.secure_field",
	-- "lock.auth_prompt", "lock.user", "lock.power", "lock.caps", "lock.slot:plugin:<id>/<widget>".
	-- "lock.power" is reserved and currently draws nothing.
	placeholder: (name: string) -> Node,

	-- Draw a widget declared in contributes.bar (pill + optional popup) or a
	-- slot declared in contributes.lock (main only; inputs are stripped).
	-- Identical trees are not re-sent, so calling this every tick is cheap.
	render: (widget: string, main: Node?, popup: Node?) -> (),
	-- Themes only: the whole lock-screen layout, built around placeholders.
	-- It must contain exactly one "lock.secure_field".
	lock_layout: (layout: Node?) -> (),
}

declare plugin: {
	id: string,
	dir: string,
}

declare log: {
	debug: (message: string) -> (),
	info: (message: string) -> (),
	warn: (message: string) -> (),
	error: (message: string) -> (),
}

-- Repeating timers run at most every 250 ms. Returns an id for timer.cancel.
declare timer: {
	every: (ms: number, callback: () -> ()) -> number,
	after: (ms: number, callback: () -> ()) -> number,
	cancel: (id: number) -> (),
}

-- The plugin's own settings, declared in contributes.settings.
declare settings: {
	get: (key: string) -> any,
	all: () -> { [string]: any },
	set: (key: string, value: boolean | number | string) -> (),
	on_change: (callback: () -> ()) -> (),
}

declare theme: {
	dark: () -> boolean,
	-- The active mode's override for a token, "#rrggbb[aa]", or nil.
	token: (name: string) -> string?,
	on_change: (callback: (dark: boolean) -> ()) -> (),
}

declare feed: {
	time: () -> {
		hour: number,
		minute: number,
		second: number,
		year: number,
		month: number,
		day: number,
		weekday: number, -- 1 = Monday
		unix: number,
	},
	-- strftime, e.g. feed.format("%H:%M")
	format: (pattern: string) -> string,
	-- cpu is the busy share since the previous call; poll it with a timer.
	system: () -> { cpu: number, memory: number, memory_used_mb: number, memory_total_mb: number, uptime: number },
	power: () -> { present: boolean, percent: number, charging: boolean },
}

-- Window events need WindowFeed (sensitive); workspace and lock events are free.
-- Events: "workspace_changed"(output, index), "lock_state"(locked),
-- "window_opened"(id, app_id, title, workspace), "window_changed"(id, app_id, title),
-- "window_closed"(id), "window_focused"(id; 0 = none).
-- run needs CompositorActions (sensitive); quit, switch-vt and spawn are refused.
declare compositor: {
	on: (event: string, callback: (...any) -> ()) -> (),
	run: ((action: string) -> ())?,
}

-- Writes are journalled and undone when the plugin is turned off.
-- `value` is RON in the key's own type: config.set("appearance", "blur_passes", "4").
-- appearance/theme/bar/lock need ThemeVisual; others need the section's capability.
declare config: {
	set: (section: string, key: string, value: string) -> (),
}

-- ThemeVisual: a path inside the package.
declare wallpaper: {
	set: (path: string) -> (),
}?

-- Network (sensitive): only the hosts the manifest lists.
declare http: {
	get: (url: string, callback: ((response: { status: number, body: string }?, error: string?) -> ())?) -> (),
	post: (url: string, body: string, callback: ((response: { status: number, body: string }?, error: string?) -> ())?) -> (),
}?

-- Exec (dangerous): only the programs the manifest lists; 10 s limit.
declare exec: {
	run: (argv: { string }, callback: ((result: { code: number, stdout: string, stderr: string }?, error: string?) -> ())?) -> (),
}?

-- FsRead / FsWrite (sensitive / dangerous). folder: "home" | "documents" |
-- "downloads" | "music" | "pictures" | "videos"; path is relative to it.
declare fs: {
	read: (folder: string, path: string, callback: (text: string?, error: string?) -> ()) -> (),
	list: (folder: string, path: string, callback: (names: { string }?, error: string?) -> ()) -> (),
	write: ((folder: string, path: string, text: string, callback: ((ok: boolean?, error: string?) -> ())?) -> ())?,
}?

-- Notify (sensitive).
declare notify: {
	send: (options: { summary: string, body: string? }) -> (),
}?

-- A script may return a module table:
-- return {
--   actions = { [name] = function() end },          -- contributes.keybinds actions
--   on_popup = function(widget: string, open: boolean) end,
--   on_unload = function() end,
-- }
