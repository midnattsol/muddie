local function raw(path)
	local parts = string.split(path, ".")
	for n = #parts, 1, -1 do
		local data = gmcp(table.concat(parts, ".", 1, n))
		if data ~= nil then
			for i = n + 1, #parts do
				if type(data) ~= "table" then return nil end
				data = data[parts[i]]
			end
			return data
		end
	end
	local reported = msdp(path)
	if reported ~= nil then return reported end
	-- Else a variable of the scripts, and the fields inside it.
	local data = vars[parts[1]]
	for i = 2, #parts do
		if type(data) ~= "table" then return nil end
		data = data[parts[i]]
	end
	return data
end
local function dig(path)
	return tonumber(path) or tonumber(raw(path))
end
local function lit(value)
	if type(value) == "string" then
		local word = string.lower(value)
		return tonumber(value) ~= 0 and word ~= "" and word ~= "false" and word ~= "no" and word ~= "off"
	end
	return value == true or (type(value) == "number" and value ~= 0) or type(value) == "table"
end
