local function greeting(hour: number): string
	if hour < 12 then
		return "Good morning"
	elseif hour < 18 then
		return "Good afternoon"
	end
	return "Good evening"
end

local function draw()
	local name = settings.get("name")
	local line = greeting(feed.time().hour) .. (if name ~= "" then ", " .. name else "")
	local power = feed.power()
	local rows = { ui.text(line, "text-xl font-medium text-text.primary") }
	if power.present then
		table.insert(rows, ui.hstack({ class = "gap-1 items-center justify-center" }, {
			ui.icon(if power.charging then "battery-charging" else "battery", "text-text.muted"),
			ui.text(string.format("%d%%", power.percent), "text-sm text-text.muted"),
		}))
	end
	ui.render("greeting", ui.vstack({ class = "gap-1 items-center" }, rows))
end

timer.every(60 * 1000, draw)
settings.on_change(draw)
compositor.on("lock_state", function(locked)
	if locked then
		draw()
	end
end)
draw()
