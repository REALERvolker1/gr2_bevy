local Fields = {}

---@param before JsonValue|nil
---@param after JsonValue|nil
---@return boolean
function Fields.equal(before, after)
	if type(before) ~= type(after) then
		return false
	end
	if type(before) ~= "table" or type(after) ~= "table" then
		return before == after
	end
	for key, value in pairs(before) do
		if not Fields.equal(value, after[key]) then
			return false
		end
	end
	for key in pairs(after) do
		if before[key] == nil then
			return false
		end
	end
	return true
end

---@param name string
---@param settings table
---@return boolean
function Fields.fast(name, settings)
	return settings.fast_fields[name] == true
end

return Fields
