local track = { title = "", artist = "", playing = false }

local function control(command: string)
	if exec then
		exec.run({ "playerctl", command }, function()
			timer.after(200, refresh)
		end)
	end
end

local function draw()
	if track.title == "" then
		ui.render("track", nil)
		return
	end
	ui.render(
		"track",
		ui.hstack({ class = "gap-1 items-center max-w-[260]" }, {
			ui.icon(if track.playing then "music" else "pause", "text-bar.fg"),
			ui.text(track.title, "text-sm font-medium truncate text-bar.fg"),
		}),
		ui.card({ class = "p-3 gap-2 w-[280]" }, {
			ui.text(track.title, "text-base font-semibold text-text.primary truncate"),
			ui.text(track.artist, "text-sm text-text.muted truncate"),
			ui.hstack({ class = "gap-2 justify-center" }, {
				ui.button({ label = "", icon = "skip-back", variant = "ghost", on_click = function() control("previous") end }),
				ui.button({ label = "", icon = if track.playing then "pause" else "play", on_click = function() control("play-pause") end }),
				ui.button({ label = "", icon = "skip-forward", variant = "ghost", on_click = function() control("next") end }),
			}),
		})
	)
end

function refresh()
	if not exec then
		return
	end
	exec.run({ "playerctl", "metadata", "--format", "{{status}}\t{{title}}\t{{artist}}" }, function(result)
		if result == nil or result.code ~= 0 then
			track = { title = "", artist = "", playing = false }
		else
			local status, title, artist = string.match(result.stdout, "^(%w+)\t([^\t]*)\t([^\n]*)")
			track = { title = title or "", artist = artist or "", playing = status == "Playing" }
		end
		draw()
	end)
end

timer.every(2000, refresh)
refresh()

return {
	actions = {
		["play-pause"] = function()
			control("play-pause")
		end,
	},
}
