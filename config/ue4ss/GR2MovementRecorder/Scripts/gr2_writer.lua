local json = require("gr2_json")
local Writer = {}
Writer.__index = Writer

---@class CaptureWriter
---@field settings table
---@field session string
---@field queue table<integer, table<string, JsonValue>>
---@field head integer
---@field tail integer
---@field lost integer
---@field total_lost integer
---@field high_water integer
---@field drain_clock_ms number
---@field sequence integer
---@field segment integer
---@field bytes integer
---@field last_flush integer
---@field output file*|nil
---@field enqueue fun(self: CaptureWriter, record: table<string, JsonValue>): boolean
---@field drain fun(self: CaptureWriter): nil
---@field close fun(self: CaptureWriter): nil

---@param settings table
---@param session string
---@return CaptureWriter
function Writer.new(settings, session)
	return setmetatable({
		settings = settings,
		session = session,
		queue = {},
		head = 0,
		tail = 0,
		lost = 0,
		total_lost = 0,
		high_water = 0,
		drain_clock_ms = 0,
		sequence = 0,
		segment = 0,
		bytes = 0,
		last_flush = os.time(),
	}, Writer)
end

-- Producer owns tail and the consumer owns head. Publish a complete copied row
-- before advancing tail; never pass a UObject or a mutable snapshot to the worker.
---@param self CaptureWriter
---@param record table<string, JsonValue>
---@return boolean
function Writer.enqueue(self, record)
	if self.tail - self.head >= self.settings.max_queued_records then
		self.lost = self.lost + 1
		self.total_lost = self.total_lost + 1
		return false
	end
	self.queue[self.tail + 1] = record
	self.tail = self.tail + 1
	self.high_water = math.max(self.high_water, self.tail - self.head)
	return true
end

---@param self CaptureWriter
---@param record table<string, JsonValue>
---@return nil
local function write(self, record)
	if self.output == nil or self.bytes >= self.settings.segment_bytes then
		self:close()
		self.segment = self.segment + 1
		local path = string.format("%s/gr2-%s-%04d.jsonl", self.settings.output_directory, self.session, self.segment)
		local existing = io.open(path, "rb")
		if existing then
			existing:close()
			error("Refuse to overwrite capture: " .. path)
		end
		self.output = assert(io.open(path, "wb"))
		local header = json.encode({
			kind = "segment",
			session = self.session,
			segment = self.segment,
			schema = "gr2-movement-v2",
		}) .. "\n"
		assert(self.output:write(header))
		self.bytes = #header
	end
	self.sequence = self.sequence + 1
	record.sequence, record.session = self.sequence, self.session
	local line = json.encode(record) .. "\n"
	assert(self.output:write(line))
	self.bytes = self.bytes + #line
end

-- Called only by LoopAsync. Lua execution still shares UE4SS's VM lock, so keep
-- worker bursts bounded; background serialization is not parallel Lua execution.
---@param self CaptureWriter
---@return nil
function Writer.drain(self)
	local started = os.clock()
	if self.lost > 0 then
		local lost = self.lost
		self.lost = 0
		write(self, { kind = "writer_loss", dropped_records = lost, invalidates_continuity = true })
	end
	for _ = 1, self.settings.writer_batch_records do
		local record = self.queue[self.head + 1]
		if record == nil then
			break
		end
		self.queue[self.head + 1] = nil
		self.head = self.head + 1
		write(self, record)
		if (os.clock() - started) * 1000 >= self.settings.writer_budget_ms then
			break
		end
	end
	if self.output and os.time() ~= self.last_flush then
		assert(self.output:flush())
		self.last_flush = os.time()
	end
	self.drain_clock_ms = (os.clock() - started) * 1000
end

---@param self CaptureWriter
---@return nil
function Writer.close(self)
	if self.output then
		assert(self.output:flush())
		assert(self.output:close())
		self.output = nil
	end
end

return Writer
