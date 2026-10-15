-- Day wallpaper from 7:00 to 19:00, night otherwise.
local function follow_sun()
	local hour = feed.time().hour
	if wallpaper then
		wallpaper.set(if hour >= 7 and hour < 19 then "wallpapers/day.svg" else "wallpapers/night.svg")
	end
end

follow_sun()
timer.every(5 * 60 * 1000, follow_sun)
