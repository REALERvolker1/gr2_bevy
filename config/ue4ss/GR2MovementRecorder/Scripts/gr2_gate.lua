local Gate = {}
Gate.__index = Gate

---@class MovementSample
---@field position number[]|nil
---@field velocity number[]|nil
---@field events JsonValue[]
---@field [string] JsonValue|nil
---@class MovementGate
---@field settings table<string, number>
---@field emit fun(record: table<string, JsonValue>)
---@field ring MovementSample[]
---@field previous MovementSample|nil
---@field active boolean
---@field quiet integer
---@field bout integer
---@field push fun(self: MovementGate, sample: MovementSample)
---@field reset fun(self: MovementGate, reason: string)

---@param settings table<string, number>
---@param emit fun(record: table<string, JsonValue>)
---@return MovementGate
function Gate.new(settings, emit)
	return setmetatable({ settings = settings, emit = emit, ring = {}, active = false, quiet = 0, bout = 0 }, Gate)
end

---@param vector number[]|nil
---@return number
local function length_squared(vector)
	if not vector then
		return 0
	end
	return vector[1] ^ 2 + vector[2] ^ 2 + vector[3] ^ 2
end

---@param self MovementGate
---@param sample MovementSample
---@return nil
function Gate.push(self, sample)
	local moved = length_squared(sample.velocity) > self.settings.speed_threshold ^ 2
	if self.previous and sample.position and self.previous.position then
		local delta = {}
		for axis = 1, 3 do
			delta[axis] = sample.position[axis] - self.previous.position[axis]
		end
		moved = moved or length_squared(delta) > self.settings.distance_threshold ^ 2
	end
	self.previous = sample
	-- Raw input edges can precede detectable motion or request a rejected action.
	-- Resource setters/checks may dispatch continuously at rest: keep those only
	-- in the bounded pre-buffer, never emit a stationary row just for their calls.
	local input_edge = false
	for _, event in ipairs(sample.events) do
		if type(event) == "table" and event.kind == "input_edge" then
			input_edge = true
		end
	end
	if not moved and not self.active and input_edge then
		self.emit({ kind = "stationary_event", sample = sample })
	end
	if moved and not self.active then
		self.active = true
		self.bout = self.bout + 1
		self.emit({ kind = "movement_start", bout = self.bout, pre_samples = #self.ring })
		for _, earlier in ipairs(self.ring) do
			self.emit({ kind = "sample", bout = self.bout, phase = "pre", sample = earlier })
		end
		self.ring = {}
	end
	if self.active then
		self.quiet = moved and 0 or self.quiet + 1
		self.emit({ kind = "sample", bout = self.bout, phase = moved and "moving" or "settling", sample = sample })
		if self.quiet >= self.settings.settle_samples then
			self.emit({ kind = "movement_end", bout = self.bout })
			self.active = false
			self.quiet = 0
		end
	else
		self.ring[#self.ring + 1] = sample
		if #self.ring > self.settings.pre_samples then
			table.remove(self.ring, 1)
		end
	end
end

---@param self MovementGate
---@param reason string
---@return nil
function Gate.reset(self, reason)
	if self.active then
		self.emit({ kind = "movement_end", bout = self.bout, reason = reason })
	end
	self.active, self.quiet, self.previous, self.ring = false, 0, nil, {}
end

return Gate
