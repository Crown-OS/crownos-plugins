-- Arithmetic without `load`: a small recursive-descent parser over + - * / ^ % and parentheses.
local function evaluate(source: string): number?
	local position = 1

	local function skip()
		while source:sub(position, position):match("%s") do
			position += 1
		end
	end

	local function peek(): string
		skip()
		return source:sub(position, position)
	end

	local expression

	local function primary(): number?
		local character = peek()
		if character == "(" then
			position += 1
			local value = expression()
			if peek() ~= ")" then
				return nil
			end
			position += 1
			return value
		elseif character == "-" then
			position += 1
			local value = primary()
			return value and -value
		end
		local number = source:match("^%d*%.?%d+", position)
		if not number then
			return nil
		end
		position += #number
		return tonumber(number)
	end

	local function power(): number?
		local base = primary()
		if base and peek() == "^" then
			position += 1
			local exponent = power()
			return exponent and base ^ exponent
		end
		return base
	end

	local function term(): number?
		local value = power()
		while value do
			local operator = peek()
			if operator ~= "*" and operator ~= "/" and operator ~= "%" then
				break
			end
			position += 1
			local right = power()
			if not right then
				return nil
			end
			value = if operator == "*" then value * right elseif operator == "/" then value / right else value % right
		end
		return value
	end

	function expression(): number?
		local value = term()
		while value do
			local operator = peek()
			if operator ~= "+" and operator ~= "-" then
				break
			end
			position += 1
			local right = term()
			if not right then
				return nil
			end
			value = if operator == "+" then value + right else value - right
		end
		return value
	end

	local value = expression()
	skip()
	if position <= #source or value == nil or value ~= value then
		return nil
	end
	return value
end

local function format(value: number): string
	if value == math.floor(value) and math.abs(value) < 1e15 then
		return string.format("%d", value)
	end
	return (string.format("%.10g", value))
end

local function encode(text: string): string
	return (text:gsub("[^%w%-%._~]", function(character)
		return string.format("%%%02X", string.byte(character))
	end))
end

local function draw()
	local system = feed.system()
	ui.render(
		"system",
		ui.vstack({ class = "gap-1 justify-center h-full" }, {
			ui.hstack({ class = "gap-2 items-center" }, {
				ui.icon("cpu", "text-text.muted"),
				ui.text(string.format("%d%%", system.cpu * 100), "text-xl font-semibold text-text.primary"),
			}),
			ui.text(
				string.format("%.1f of %.1f GB", system.memory_used_mb / 1024, system.memory_total_mb / 1024),
				"text-sm text-text.muted"
			),
		})
	)
end

local tick: number? = nil

draw()

return {
	on_launcher = function(shown: boolean)
		if tick then
			timer.cancel(tick)
			tick = nil
		end
		if shown then
			draw()
			tick = timer.every(2000, draw)
		end
	end,

	on_search = function(provider: string, query: string)
		if provider == "calc" then
			local value = if query:match("[%d%)]") and query:match("[%+%-%*/%^%%]") then evaluate(query) else nil
			launcher.results(provider, query, if value
				then { { id = format(value), title = "= " .. format(value), subtitle = query, icon = "calculator" } }
				else {})
		elseif provider == "web" then
			launcher.results(provider, query, {
				{ id = query, title = "Search the web for “" .. query .. "”", subtitle = "duckduckgo.com", icon = "globe" },
			})
		end
	end,

	on_activate = function(provider: string, id: string)
		if provider == "web" and exec then
			exec.run({ "xdg-open", "https://duckduckgo.com/?q=" .. encode(id) })
		end
	end,
}
