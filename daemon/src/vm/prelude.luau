local ui = {}

local function container(kind)
	return function(props, children)
		props = props or {}
		local node = { kind = kind, class = props.class, children = {} }
		for _, child in ipairs(children or {}) do
			table.insert(node.children, child)
		end
		for _, child in ipairs(props) do
			table.insert(node.children, child)
		end
		return node
	end
end

ui.vstack = container("vstack")
ui.hstack = container("hstack")
ui.zstack = container("zstack")
ui.card = container("card")

function ui.text(text, class)
	return { kind = "text", text = tostring(text), class = class }
end

function ui.icon(name, class)
	return { kind = "icon", name = name, class = class }
end

function ui.image(src, class)
	return { kind = "image", src = src, class = class }
end

function ui.spacer()
	return { kind = "spacer" }
end

function ui.divider()
	return { kind = "divider" }
end

function ui.badge(text, tone)
	return { kind = "badge", text = tostring(text), tone = tone }
end

function ui.progress(value, class)
	return { kind = "progress", value = value, class = class }
end

function ui.placeholder(name)
	return { kind = "placeholder", name = name }
end

function ui.button(props)
	return {
		kind = "button",
		label = props.label or "",
		icon = props.icon,
		variant = props.variant,
		class = props.class,
		on_click = props.on_click,
	}
end

function ui.toggle(props)
	return { kind = "toggle", label = props.label or "", on = props.on == true, on_change = props.on_change }
end

function ui.slider(props)
	return {
		kind = "slider",
		value = props.value or 0,
		min = props.min or 0,
		max = props.max or 1,
		step = props.step or 0.01,
		on_change = props.on_change,
	}
end

function ui.select(props)
	return { kind = "select", options = props.options or {}, selected = props.selected or 1, on_change = props.on_change }
end

return ui
