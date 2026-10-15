local history = {}

local function draw()
	local cpu = feed.system().cpu
	table.insert(history, cpu)
	if #history > 30 then
		table.remove(history, 1)
	end

	local pill = { ui.icon("cpu", "text-bar.fg") }
	if settings.get("show_label") then
		table.insert(pill, ui.text(string.format("%d%%", cpu * 100), "text-sm font-semibold text-bar.fg"))
	end

	ui.render(
		"status",
		ui.hstack({ class = "gap-1 items-center" }, pill),
		ui.card({ class = "p-3 gap-2 w-[220]" }, {
			ui.text("CPU", "text-sm font-semibold text-text.primary"),
			ui.progress(cpu),
			ui.text(string.format("%d samples", #history), "text-xs text-text.muted"),
		})
	)
end

local tick = timer.every(settings.get("interval") * 1000, draw)

settings.on_change(function()
	timer.cancel(tick)
	tick = timer.every(settings.get("interval") * 1000, draw)
	draw()
end)

draw()

return {
	on_popup = function(widget, open)
		if open then
			draw()
		end
	end,
}
