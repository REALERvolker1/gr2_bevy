local json = {}

---@class JsonObject
---@field [string] boolean|number|string|JsonObject|JsonArray|nil
---@alias JsonArray (boolean|number|string|JsonObject|JsonArray)[]
---@alias JsonValue boolean|number|string|JsonObject|JsonArray

---@param value string
---@return string
local function quote(value)
	---@param char string
	---@return string
	local function escape(char)
		local escapes = { ['"'] = '\\"', ["\\"] = "\\\\", ["\n"] = "\\n", ["\r"] = "\\r", ["\t"] = "\\t" }
		return escapes[char] or string.format("\\u%04x", string.byte(char))
	end
	return '"' .. value:gsub('[%z\1-\31\\"]', escape) .. '"'
end

---@param value JsonValue|nil
---@return string
function json.encode(value)
	local kind = type(value)
	if kind == "nil" then
		return "null"
	elseif kind == "string" then
		return quote(value)
	elseif kind == "boolean" then
		return tostring(value)
	elseif kind == "number" then
		if value ~= value or value == math.huge or value == -math.huge then
			return "null"
		end
		return string.format("%.17g", value)
	elseif kind ~= "table" then
		error("JSON encoder accepts copied values only")
	end
	local parts = {}
	if #value > 0 then
		for index = 1, #value do
			parts[#parts + 1] = json.encode(value[index])
		end
		return "[" .. table.concat(parts, ",") .. "]"
	end
	local keys = {}
	for key in pairs(value) do
		assert(type(key) == "string", "JSON object keys must be strings")
		keys[#keys + 1] = key
	end
	table.sort(keys)
	for _, key in ipairs(keys) do
		parts[#parts + 1] = quote(key) .. ":" .. json.encode(value[key])
	end
	return "{" .. table.concat(parts, ",") .. "}"
end

return json
