-- Waiting: what waits is the coroutine each function of a script is run in,
-- which goes on when what it waits for comes, or the time it gives is up.
local next_line, matcher, STOPPED = ...

local function only(options, known)
	for key in options do
		if not table.find(known, key) then
			error(`there is no option {key} here; there are {table.concat(known, ", ")}`, 4)
		end
	end
end

-- Stops until what `start` makes calls the function it is given, with what it
-- gives it; or, after `timeout` seconds if there are, with nothing.
local function stop(timeout, start)
	if not coroutine.isyieldable() then
		error(
			"nothing can wait here: in on(\"send\", ...), in a module being required or in a function given to another, as to table.sort",
			3
		)
	end
	local thread = coroutine.running()
	local made, timer
	local function go(...)
		if made then
			made:remove()
		end
		if timer then
			timer:remove()
		end
		local ok, wrong = coroutine.resume(thread, ...)
		if not ok then
			error(wrong, 0)
		end
	end
	made = start(go)
	if timeout then
		timer = after(timeout, function()
			go(nil)
		end)
	end
	return coroutine.yield(STOPPED)
end

local function wait(what, name, options)
	if type(what) == "number" then
		return stop(what, function() end)
	end
	if what == "send" then
		error("what is sent does not wait: there is on(\"send\", ...)", 2)
	end
	if type(name) == "table" and options == nil then
		name, options = nil, name
	end
	options = options or {}
	if what == "line" then
		only(options, { "timeout", "prompt" })
		return stop(options.timeout, function(go)
			return next_line(name, options.prompt, go)
		end)
	end
	local timeout = options.timeout
	local rest = table.clone(options)
	rest.timeout = nil
	return stop(timeout, function(go)
		local function came(data, ...)
			go(if data == nil then true else data, ...)
		end
		if name == nil then
			return on(what, came, rest)
		end
		return on(what, name, came, rest)
	end)
end

local function lines(options)
	options = options or {}
	only(options, { "till", "count", "timeout" })
	if next(options) == nil then
		error("lines needs till, count or timeout, to end", 2)
	end
	local ends = options.till and matcher(options.till)
	local left = options.count or math.huge
	return function()
		if left <= 0 then
			return nil
		end
		left -= 1
		local m, line = wait("line", "", { timeout = options.timeout, prompt = false })
		if not m or (ends and ends(line.text)) then
			left = 0
			return nil
		end
		return line
	end
end

return wait, lines
