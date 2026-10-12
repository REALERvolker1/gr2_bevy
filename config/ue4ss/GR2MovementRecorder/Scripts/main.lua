-- Read-only UE4SS v3.0.1 observer. No input injection, setters, raw offsets,
-- native detours, or return-value overrides. Poll timing is not engine tick identity.
---@class CaptureObject
---@field [string] unknown
---@field IsValid fun(self: CaptureObject): boolean
---@field GetAddress fun(self: CaptureObject): integer
---@field GetFullName fun(self: CaptureObject): string
---@field GetFName fun(self: CaptureObject): CaptureName
---@field GetClass fun(self: CaptureObject): CaptureStruct
---@field GetOuter fun(self: CaptureObject): CaptureObject
---@field GetWorld fun(self: CaptureObject): CaptureObject
---@field GetPropertyValue fun(self: CaptureObject, name: string): unknown
---@field type fun(self: CaptureObject): string
---@field HasAnyFlags fun(self: CaptureObject, flags: integer): boolean
---@class CaptureEnum: CaptureObject
---@field ForEachName fun(self: CaptureEnum, callback: fun(name: CaptureName, value: integer))
---@class CaptureStruct: CaptureObject
---@field GetSuperStruct fun(self: CaptureStruct): CaptureStruct
---@field ForEachProperty fun(self: CaptureStruct, callback: fun(property: CaptureObject))
---@field ForEachFunction fun(self: CaptureStruct, callback: fun(func: CaptureFunction))
---@class CaptureFunction: CaptureObject
---@field GetFunctionFlags fun(self: CaptureFunction): integer
---@class CaptureName
---@field ToString fun(self: CaptureName): string
---@class CaptureParam
---@field get fun(self: CaptureParam): unknown
---@class WatchedObject
---@field object CaptureObject
---@field name string
---@field fields string[]
---@field links string[]
---@field fast string[]
---@field id string
---@field upgrades boolean
---@field stamina boolean
---@field previous table<string, JsonValue>
---@field context table<string, JsonValue>

local source = debug.getinfo(1, "S").source:gsub("^@", "")
local directory = assert(source:match("^(.*)[/\\]"), "Recorder script directory missing")
package.path = directory .. "/?.lua;" .. package.path
local Writer = require("gr2_writer")
local Fields = require("gr2_fields")
local Gate = require("gr2_gate")
local settings = require("gr2_settings")

---@type fun(name: string): CaptureObject[]|nil
local find_all = FindAllOf
---@type fun(name: string): CaptureObject
local find_static = StaticFindObject
---@type fun(callback: fun())
local game_thread = ExecuteInGameThread
---@type fun(milliseconds: integer, callback: fun(): boolean)
local loop_async = LoopAsync
---@type fun(path: string, pre: fun(context: CaptureParam, ...: CaptureParam), post?: fun(context: CaptureParam, ...: CaptureParam)): integer, integer
local register_hook = RegisterHook
---@type fun(name: string): CaptureName
local make_name = FName
---@type table<string, {KeyName: CaptureName}>
local input_key_objects = {}
---@type table<string, boolean>
local previous_keys = {}
for _, keys in ipairs({ settings.input_keys, settings.input_axes }) do
	for _, key in ipairs(keys) do
		input_key_objects[key] = { KeyName = make_name(key) }
	end
end

local session = tostring(os.date("!%Y%m%dT%H%M%SZ"))
local polls, generation = 0, 0
local writer = Writer.new(settings, session)
---@type table<integer, string[]>
local struct_members = {}
---@type table<integer, string>
local object_names = {}
local context_object, context_field, context_revision = 1, 1, 0
local context_complete, next_context = false, 0
local upgrade_revision, next_upgrade = 0, 0
---@type table<string, JsonValue>|nil
local upgrade_state
local upgrades_known = false
local metric_count, metric_sum, metric_max, next_metric = 0, 0, 0, 0
---@type JsonValue[]
local events = {}
local stamina_dash_path = "/Script/Ghostrunner.StaminaWorldSubsystem:TryUseStaminaForDash"
local next_dash_call = 0
---@type table<integer, integer[]>
local dash_calls = {}
---@type WatchedObject[]
local watched = {}
---@type table<integer, boolean>
local receivers = {}
---@type table<string, boolean>
local hooked = {}
---@type table<string, boolean>
local reported = {}
---@type CaptureObject|nil
local pawn, controller, world, clock
local hook_count, lost_events = 0, 0
local pending, disabled, in_sample = false, false, false
local first_sample_finished = false
---@type number|nil
local last_world_time

---@param object CaptureObject|nil
---@return boolean
local function valid(object)
	if object == nil then
		return false
	end
	local ok, result = pcall(function()
		return object:IsValid() and not object:HasAnyFlags(0x18030)
	end)
	return ok and result == true
end

---@param object unknown
---@param name string|integer
---@return unknown
local function read(object, name)
	local ok, value = pcall(function()
		return object[name]
	end)
	if ok then
		return value
	end
	return nil
end

---@param value unknown
---@return number[]|nil
local function vector(value)
	local x, y, z = read(value, "X"), read(value, "Y"), read(value, "Z")
	if type(x) == "number" and type(y) == "number" and type(z) == "number" then
		return { x, y, z }
	end
	return nil
end

local struct_fields = {
	"X",
	"Y",
	"Z",
	"Pitch",
	"Yaw",
	"Roll",
	"bBlockingHit",
	"bStartPenetrating",
	"bWalkableFloor",
	"bLineTrace",
	"FloorDist",
	"LineDist",
	"HitResult",
	"Time",
	"Distance",
	"Location",
	"ImpactPoint",
	"Normal",
	"ImpactNormal",
	"TraceStart",
	"TraceEnd",
	"PenetrationDepth",
	"Actor",
	"Component",
	"BoneName",
	"FaceIndex",
	"Type",
	"State",
	"bIsBoosted",
	"IsAvailable",
	"AvailableOnTier",
	"ShardCost",
	"DataCost",
	"BoostCondition",
	"RequiredTier",
	"Branches",
	"Sockets",
	"KeyName",
	"Translation",
	"Rotation",
	"Scale3D",
}

---@param value unknown
---@param depth integer
---@return JsonValue|nil
local function copy(value, depth)
	local kind = type(value)
	if kind == "boolean" or kind == "number" or kind == "string" then
		return value
	elseif value == nil or depth > 5 then
		return nil
	end
	-- UE4SS __index is a native reflection operation, not a harmless Lua
	-- capability probe. Dispatch on its documented type before touching members.
	local wrapper
	if kind == "userdata" then
		wrapper = value:type()
	elseif kind == "table" then
		local type_method = rawget(value, "type")
		if type(type_method) == "function" then
			wrapper = type_method(value)
		end
	else
		return nil
	end
	if wrapper == "UObject" or wrapper == "UClass" or wrapper == "UStruct" or wrapper == "UFunction" then
		if value:IsValid() then
			local address = value:GetAddress()
			local name = object_names[address]
			if name == nil then
				name = value:GetFullName()
				object_names[address] = name
			end
			return { object = name }
		end
		return { unavailable = true, type = wrapper }
	elseif wrapper == "FName" or wrapper == "FText" then
		return value:ToString()
	elseif wrapper == "TArray" then
		local count = value:GetArrayNum()
		local entries = {}
		for index = 1, math.min(count, 256) do
			local entry = copy(read(value, index), depth + 1)
			entries[#entries + 1] = entry == nil and { unavailable = true } or entry
		end
		return { count = count, entries = entries, truncated = count > 256 }
	elseif wrapper ~= nil and wrapper ~= "UScriptStruct" then
		return { unavailable = true, type = tostring(wrapper) }
	end
	local members = struct_fields
	if wrapper == "UScriptStruct" then
		local address = value:GetPropertyAddress()
		members = struct_members[address]
		if members == nil then
			local present = {}
			---@param property CaptureObject
			---@return nil
			local function member_info(property)
				present[property:GetFName():ToString()] = true
			end
			local metadata = value
			for _ = 1, 12 do
				if not metadata:IsValid() then
					break
				end
				metadata:ForEachProperty(member_info)
				metadata = metadata:GetSuperStruct()
			end
			members = {}
			for _, key in ipairs(struct_fields) do
				if present[key] then
					members[#members + 1] = key
				end
			end
			struct_members[address] = members
		end
	end
	local fields = {}
	for _, key in ipairs(members) do
		if wrapper ~= nil or rawget(value, key) ~= nil then
			local member = copy(read(value, key), depth + 1)
			if member ~= nil then
				fields[key] = member
			end
		end
	end
	return next(fields) and fields or nil
end

---@param record table<string, JsonValue>
---@return nil
local function emit(record)
	if not disabled then
		writer:enqueue(record)
	end
end

---@param key string
---@param message string
---@return nil
local function report(key, message)
	if reported[key] then
		return
	end
	reported[key] = true
	emit({ kind = "diagnostic", key = key, message = message })
end

---@return number|nil
local function world_time()
	if not valid(pawn) or not clock then
		return nil
	end
	local ok, result = pcall(function()
		return clock:GetTimeSeconds(pawn)
	end)
	return ok and type(result) == "number" and result or nil
end

---@return number|nil
local function world_audio_time()
	if not valid(pawn) or not clock then
		return nil
	end
	local ok, result = pcall(function()
		return clock:GetAudioTimeSeconds(pawn)
	end)
	if not ok or type(result) ~= "number" then
		report("audio_clock", "GetAudioTimeSeconds unavailable; dash clock comparison remains unknown")
		return nil
	end
	return result
end

local action_terms = {
	"dash",
	"slide",
	"jump",
	"land",
	"wall",
	"mount",
	"mantle",
	"vault",
	"climb",
	"grap",
	"gapjam",
	"fall",
	"crouch",
	"sprint",
	"stamina",
	"energy",
	"sensory",
	"boost",
	"die",
	"death",
	"respawn",
	"checkpoint",
	"upgrade",
	"input",
	"move",
	"hit",
	"collis",
	"teleport",
	"launch",
	"motor",
	"ride",
}

---@param name string
---@return boolean
local function relevant(name)
	local lower = name:lower()
	for _, term in ipairs(action_terms) do
		if lower:find(term, 1, true) then
			return true
		end
	end
	return false
end

---@param context CaptureParam
---@param path string
---@param phase string
---@param ... CaptureParam
---@return nil
local function action(context, path, phase, ...)
	if disabled or in_sample then
		return
	end
	local object = context:get()
	if not valid(object) or not receivers[object:GetAddress()] then
		return
	end
	local call_id
	if path == stamina_dash_path then
		local address = object:GetAddress()
		local stack = dash_calls[address] or {}
		dash_calls[address] = stack
		if phase == "pre" then
			next_dash_call = next_dash_call + 1
			call_id = next_dash_call
			stack[#stack + 1] = call_id
		else
			call_id = table.remove(stack)
		end
	end
	-- Bound pathological per-frame dispatch while retaining explicit loss counters.
	if #events >= settings.max_pending_events then
		lost_events = lost_events + 1
		return
	end
	local parameters = {}
	for index = 1, select("#", ...) do
		local param = select(index, ...)
		local ok, value = pcall(function()
			return param:get()
		end)
		local copied = ok and copy(value, 0)
		if copied == nil or not ok then
			copied = { unavailable = true }
		end
		parameters[#parameters + 1] = copied
	end
	local event = {
		function_path = path,
		phase = phase,
		receiver = object:GetFullName(),
		parameters = parameters,
		world_time = world_time(),
		after_poll = polls,
		generation = generation,
	}
	if path == stamina_dash_path then
		-- Observe the reflected request boundary, before polling can mix in regeneration.
		-- No return override: acceptance must be inferred from the captured state.
		local ok, value = pcall(function()
			return object:GetStamina()
		end)
		event.resource_snapshot = {
			stamina = ok and copy(value, 0) or { unavailable = true },
			dash_last_time_used = copy(read(object, "DashLastTimeUsed"), 0),
			dash_cooldown = copy(read(object, "DashCooldown"), 0),
			dash_base_cost = copy(read(object, "DashStaminaCost"), 0),
			world_audio_time = world_audio_time(),
			upgrade_revision = upgrade_revision,
			upgrades_known = upgrades_known,
		}
		event.call_id = call_id
		event.pair_known = call_id ~= nil
	end
	events[#events + 1] = event
end

---@param func CaptureFunction
---@return nil
local function hook(func)
	local path = func:GetFullName():gsub("^%S+%s+", "")
	local targeted = settings.enable_stamina_dash_hook and path == stamina_dash_path
	if not settings.enable_hooks and not targeted then
		return
	end
	local name = func:GetFName():ToString()
	local lower = name:lower()
	if
		not targeted
		and (
			not relevant(name)
			or lower:match("^get")
			or lower:match("^is")
			or lower:match("^can")
			or lower:match("^update")
			or lower:match("^calculate")
			or lower:match("^compute")
			or lower:find("tick")
		)
	then
		return
	end
	if hooked[path] then
		return
	end
	hooked[path] = true
	-- UE4SS does not support delegate signatures. FUNC_Delegate and
	-- FUNC_MulticastDelegate must never be passed to RegisterHook.
	if func:GetFunctionFlags() & 0x110000 ~= 0 then
		emit({ kind = "hook_skipped", path = path, reason = "delegate_signature" })
		return
	end
	if hook_count >= settings.max_hooks then
		report("hook_limit", "Hook capacity reached; inspect schema for omitted paths")
		return
	end
	---@param context CaptureParam
	---@param ... CaptureParam
	---@return nil
	local function pre(context, ...)
		local ok, err = pcall(action, context, path, path:match("^/Script/") and "pre" or "post", ...)
		if not ok then
			report("callback:" .. path, tostring(err))
		end
	end
	---@param context CaptureParam
	---@param ... CaptureParam
	---@return nil
	local function post(context, ...)
		local ok, err = pcall(action, context, path, "post", ...)
		if not ok then
			report("callback:" .. path, tostring(err))
		end
	end
	local ok, pre_id, post_id = pcall(register_hook, path, pre, path:match("^/Script/") and post or nil)
	if ok then
		hook_count = hook_count + 1
		-- v3.0.1's Lua UFunction wrapper has no ForEachProperty method.
		-- Callback values remain ordered, but parameter names are unavailable.
		emit({
			kind = "hook_registered",
			path = path,
			pre_id = pre_id,
			post_id = post_id,
			parameter_names = "unavailable_v301_lua_api",
		})
	else
		emit({ kind = "hook_failed", path = path, message = tostring(pre_id) })
	end
end

---@class CapturePlan
---@field fields string[]
---@field fast string[]
---@field links string[]
---@field schema JsonValue[]
---@field upgrades boolean
---@field stamina boolean
---@type table<integer, CapturePlan>
local class_plans = {}

---@param object CaptureObject
---@return CapturePlan
local function plan(object)
	local class = object:GetClass()
	local address = class:GetAddress()
	local cached = class_plans[address]
	if cached then
		return cached
	end
	local result = { fields = {}, fast = {}, links = {}, schema = {}, stamina = false, upgrades = false }
	local names = {}
	for _ = 1, 12 do
		if not class:IsValid() then
			break
		end
		---@param property CaptureObject
		---@return nil
		local function property_info(property)
			local name = property:GetFName():ToString()
			if names[name] or #result.fields >= settings.max_fields then
				return
			end
			names[name] = true
			local typename = property:GetFullName():match("^(%S+)") or "unknown"
			result.schema[#result.schema + 1] = { name = name, type = typename }
			if typename:find("ObjectProperty") then
				-- Broad objects remain context identities, but only approved
				-- runtime components can enter recursive discovery.
				result.fields[#result.fields + 1] = name
				if settings.component_links[name] then
					result.links[#result.links + 1] = name
				end
			elseif not typename:find("Delegate") and not typename:find("TextProperty") then
				result.fields[#result.fields + 1] = name
				if Fields.fast(name, settings) then
					result.fast[#result.fast + 1] = name
				end
			end
		end
		if class:GetFullName() == "Class /Script/Ghostrunner.StaminaWorldSubsystem" then
			result.stamina = true
		end
		if class:GetFullName() == "Class /Script/Ghostrunner.UpgradesSubsystem" then
			result.upgrades = true
		end
		class:ForEachProperty(property_info)
		if
			settings.enable_hooks
			or (
				settings.enable_stamina_dash_hook
				and class:GetFullName() == "Class /Script/Ghostrunner.StaminaWorldSubsystem"
			)
		then
			class:ForEachFunction(hook)
		end
		class = class:GetSuperStruct()
	end
	class_plans[address] = result
	return result
end

---@param object CaptureObject
---@param depth integer
---@return nil
local function watch(object, depth)
	if not valid(object) or receivers[object:GetAddress()] or #watched >= settings.max_objects then
		return
	end
	receivers[object:GetAddress()] = true
	local metadata = plan(object)
	local item = {
		object = object,
		name = object:GetFullName(),
		id = tostring(#watched + 1),
		fields = metadata.fields,
		links = metadata.links,
		fast = metadata.fast,
		stamina = metadata.stamina,
		upgrades = metadata.upgrades,
		previous = {},
		context = {},
	}
	watched[#watched + 1] = item
	emit({
		kind = "object_schema",
		generation = generation,
		id = item.id,
		object = item.name,
		class = object:GetClass():GetFullName(),
		outer = object:GetOuter():GetFullName(),
		properties = metadata.schema,
		fast_fields = item.fast,
	})
	if depth < 2 then
		for _, name in ipairs(item.links) do
			local child = read(object, name)
			if valid(child) then
				watch(child, depth + 1)
			end
		end
	end
end

local gate = Gate.new(settings, emit)

---@param value JsonValue|nil
---@return boolean
local function complete_array(value)
	return type(value) == "table"
		and type(value.count) == "number"
		and type(value.entries) == "table"
		and #value.entries == value.count
		and value.truncated == false
end

---@param board JsonValue|nil
---@return boolean
local function complete_board(board)
	if type(board) ~= "table" then
		return false
	end
	local branches = board.Branches
	if type(branches) ~= "table" or not complete_array(branches) then
		return false
	end
	local branch_entries = branches.entries
	if type(branch_entries) ~= "table" then
		return false
	end
	for _, branch in ipairs(branch_entries) do
		if type(branch) ~= "table" then
			return false
		end
		local sockets = branch.Sockets
		if type(sockets) ~= "table" or not complete_array(sockets) then
			return false
		end
		local socket_entries = sockets.entries
		if type(socket_entries) ~= "table" then
			return false
		end
		for _, socket in ipairs(socket_entries) do
			if
				type(socket) ~= "table"
				or type(socket.Type) ~= "number"
				or type(socket.State) ~= "number"
				or type(socket.bIsBoosted) ~= "boolean"
			then
				return false
			end
		end
	end
	return true
end

-- Loadout is sampled independently of motion. Only baseline/change records
-- reach the writer; a menu loadout change cannot inherit an earlier revision.
---@param time number|nil
---@param force? boolean
---@return nil
local function capture_upgrades(time, force)
	local runtime = os.clock()
	if not force and runtime < next_upgrade then
		return
	end
	next_upgrade = runtime + settings.upgrade_poll_interval
	---@type table<string, JsonValue>
	local state = { available = false, reason = "world_upgrade_subsystem_unavailable" }
	for _, item in ipairs(watched) do
		if item.upgrades and valid(item.object) then
			local board = copy(read(item.object, "MotherboardInfo"), 0)
			local available = complete_board(board)
			state = {
				available = available,
				id = item.id,
				motherboard = board or false,
				core_level = copy(read(item.object, "CurrentCoreLevel"), 0) or false,
				upgrade_database = copy(read(item.object, "UpgradesDatabase"), 0) or false,
				value_database = copy(read(item.object, "UpgradesValuesDatabase"), 0) or false,
			}
			if not available then
				state.reason = "motherboard_fields_incomplete"
			end
			break
		end
	end
	if not Fields.equal(upgrade_state, state) then
		upgrade_revision = upgrade_revision + 1
		upgrade_state = state
		upgrades_known = state.available == true
		emit({
			kind = "upgrades",
			generation = generation,
			revision = upgrade_revision,
			world_time = time or false,
			initial = upgrade_revision == 1,
			state = state,
			phase = "poll",
			poll_interval = settings.upgrade_poll_interval,
			poll_clock = "os.clock; independent of paused world time",
		})
	end
end

---@return boolean
local function acquire()
	for _, candidate in ipairs(find_all("PlayerController") or {}) do
		local player = read(candidate, "Pawn")
		if valid(candidate) and valid(player) then
			local ok, is_character = pcall(function()
				return player:IsA("/Script/Engine.Character") and player:IsPlayerControlled()
			end)
			if ok and is_character then
				controller, pawn, world = candidate, player, player:GetWorld()
				generation = generation + 1
				dash_calls = {}
				watched, receivers, events, previous_keys = {}, {}, {}, {}
				last_world_time = nil
				class_plans, struct_members, object_names = {}, {}, {}
				context_object, context_field, context_revision = 1, 1, 0
				context_complete, next_context = false, 0
				upgrade_revision, next_upgrade, upgrade_state, upgrades_known = 0, 0, nil, false
				gate:reset("player_changed")
				emit({
					kind = "player",
					generation = generation,
					pawn = pawn:GetFullName(),
					controller = controller:GetFullName(),
					world = world:GetFullName(),
				})
				watch(pawn, 0)
				watch(controller, 0)
				for _, name in ipairs({ "StaminaWorldSubsystem", "EnergySubsystem", "UpgradesSubsystem" }) do
					for _, subsystem in ipairs(find_all(name) or {}) do
						if valid(subsystem) and subsystem:GetOuter():GetAddress() == world:GetAddress() then
							watch(subsystem, 0)
						end
					end
				end
				capture_upgrades(world_time(), true)
				print("[GR2MovementRecorder] player acquired; generation=" .. generation .. "\n")
				return true
			end
		end
	end
	return false
end

---@param time number|nil
---@return table<string, JsonValue>
local function inputs(time)
	local keys, axes = {}, {}
	for _, key in ipairs(settings.input_keys) do
		local ok, down = pcall(function()
			return controller:IsInputKeyDown(input_key_objects[key])
		end)
		if ok and type(down) == "boolean" then
			keys[key] = down
			if previous_keys[key] ~= nil and previous_keys[key] ~= down then
				events[#events + 1] = { kind = "input_edge", key = key, down = down, world_time = time, phase = "poll" }
			end
			previous_keys[key] = down
		else
			report("input_keys", "Raw key query unavailable; use hooked actions and reflected input fields")
		end
	end
	for _, key in ipairs(settings.input_axes) do
		local ok, value = pcall(function()
			return controller:GetInputAnalogKeyState(input_key_objects[key])
		end)
		if ok and type(value) == "number" then
			axes[key] = value
		else
			report("input_axes", "Analog key query unavailable")
		end
	end
	return { keys = keys, axes = axes }
end

-- Flush only the first sample's boundaries so a blocked native call leaves a
-- usable locator even when the process must be terminated.
---@param phase string
---@param object_name? string
---@return nil
local function sample_phase(phase, object_name)
	if settings.diagnose_first_sample and not first_sample_finished then
		emit({ kind = "sampling_probe", phase = phase, object = object_name or "" })
	end
end

-- Context is read in bounded batches, never a full snapshot on one frame.
-- Initial context is setup evidence; repeat cycles run only during motion.
---@param time number|nil
---@param moving boolean
---@return nil
local function context_sample(time, moving)
	if context_complete and (not moving or time == nil or time < next_context) then
		return
	end
	if context_object > #watched then
		context_object, context_field = 1, 1
		context_complete = true
		next_context = (time or 0) + settings.context_interval
		return
	end
	local patch, item = {}, watched[context_object]
	if not valid(item.object) then
		context_object, context_field = context_object + 1, 1
		return
	end
	local removed = {}
	for _ = 1, settings.context_fields_per_sample do
		local name = item.fields[context_field]
		if name == nil then
			context_object, context_field = context_object + 1, 1
			break
		end
		local value = copy(read(item.object, name), 0)
		if not Fields.equal(item.context[name], value) then
			item.context[name] = value
			if value == nil then
				removed[#removed + 1] = name
			else
				patch[name] = value
			end
		end
		context_field = context_field + 1
	end
	if next(patch) or #removed > 0 then
		context_revision = context_revision + 1
		emit({
			kind = "context_delta",
			generation = generation,
			id = item.id,
			revision = context_revision,
			world_time = time or false,
			values = patch,
			removed = removed,
			initial = not context_complete,
		})
	end
end

---@return nil
local function sample()
	local started = os.clock()
	polls = polls + 1
	local current_pawn = read(controller, "Pawn")
	local previous_address = pawn and pawn:GetAddress()
	if
		not valid(pawn)
		or not valid(controller)
		or not valid(current_pawn)
		or current_pawn:GetAddress() ~= previous_address
	then
		gate:reset("player_unavailable")
		pawn = nil
		if polls % 125 ~= 1 or not acquire() then
			return
		end
	end
	local player = assert(pawn)
	if polls % 125 == 0 then
		-- State components can appear after possession. Discover only links from
		-- already selected player objects; never sweep unrelated actors per frame.
		local count = #watched
		for index = 1, count do
			local item = watched[index]
			if valid(item.object) then
				for _, name in ipairs(item.links) do
					local child = read(item.object, name)
					if valid(child) then
						watch(child, 1)
					end
				end
			end
		end
	end
	sample_phase("world_clock")
	local time = world_time()
	local audio_time = world_audio_time()
	capture_upgrades(time)
	-- Deduplicate stalled/paused observations only when the world clock exists.
	if time ~= nil and time == last_world_time and #events == 0 then
		return
	end
	local delta = time and last_world_time and time - last_world_time or nil
	last_world_time = time
	in_sample = true
	sample_phase("input")
	local input = inputs(time)
	local state = {}
	for _, item in ipairs(watched) do
		if #item.fast > 0 or item.stamina then
			if valid(item.object) then
				local fields = {}
				for _, name in ipairs(item.fast) do
					local value = copy(read(item.object, name), 0)
					fields[name] = value
					if
						item.previous[name] ~= nil
						and (type(value) == "boolean" or name == "MovementMode" or name == "CustomMovementMode" or name == "JumpCounter" or name == "CurrentJumpType" or name == "DashCharges")
						and not Fields.equal(item.previous[name], value)
					then
						if #events < settings.max_pending_events then
							events[#events + 1] = {
								kind = "state_transition",
								id = item.id,
								field = name,
								before = item.previous[name],
								after = value,
								phase = "poll",
								world_time = time,
							}
						else
							lost_events = lost_events + 1
						end
					end
				end
				if item.stamina then
					fields["@GetStamina"] = copy(item.object:GetStamina(), 0)
				end
				item.previous = fields
				state[item.id] = fields
			end
		end
	end
	sample_phase("kinematics")
	local position_ok, position = pcall(function()
		return player:K2_GetActorLocation()
	end)
	local velocity_ok, velocity = pcall(function()
		return player:GetVelocity()
	end)
	local rotation_ok, rotation = pcall(function()
		return player:K2_GetActorRotation()
	end)
	local control_ok, control_rotation = pcall(function()
		return controller:GetControlRotation()
	end)
	local location, speed = position_ok and vector(position) or nil, velocity_ok and vector(velocity) or nil
	local moving = speed ~= nil and (speed[1] ^ 2 + speed[2] ^ 2 + speed[3] ^ 2 > settings.speed_threshold ^ 2)
	if gate.previous and location and gate.previous.position then
		local prior = assert(gate.previous.position)
		moving = moving
			or (
				(location[1] - prior[1]) ^ 2 + (location[2] - prior[2]) ^ 2 + (location[3] - prior[3]) ^ 2
				> settings.distance_threshold ^ 2
			)
	end
	context_sample(time, moving or gate.active)
	in_sample = false
	if not position_ok or not velocity_ok then
		report(
			"kinematics",
			"Location/velocity getter unavailable; inspect trace and do not accept as a movement baseline"
		)
	end
	local sample_record = {
		poll = polls,
		generation = generation,
		world_time = time,
		world_audio_time = audio_time,
		world_delta = delta,
		wall_seconds = os.time(),
		position = location,
		velocity = speed,
		context_revision = context_revision,
		upgrade_revision = upgrade_revision,
		upgrades_known = upgrades_known,
		actor_rotation = rotation_ok and copy(rotation, 0) or nil,
		control_rotation = control_ok and copy(control_rotation, 0) or nil,
		objects = state,
		input = input,
		events = events,
		lost_events = lost_events,
	}
	events = {}
	local elapsed = (os.clock() - started) * 1000
	sample_record.capture_clock_ms = elapsed
	metric_count, metric_sum, metric_max = metric_count + 1, metric_sum + elapsed, math.max(metric_max, elapsed)
	if (moving or gate.active) and time and time >= next_metric then
		emit({
			kind = "performance",
			generation = generation,
			world_time = time,
			capture_clock_mean_ms = metric_sum / metric_count,
			capture_clock_max_ms = metric_max,
			observations = metric_count,
			queued_records = writer.tail - writer.head,
			queue_high_water = writer.high_water,
			writer_clock_ms = writer.drain_clock_ms,
			writer_dropped_total = writer.total_lost,
			watched_objects = #watched,
			phase = "os.clock duration; platform-dependent wall/CPU semantics",
		})
		metric_count, metric_sum, metric_max, next_metric = 0, 0, 0, time + settings.metrics_interval
	end
	gate:push(sample_record)
	sample_phase("complete")
	first_sample_finished = true
end

---@return nil
local function scheduled_sample()
	local ok, err = pcall(sample)
	in_sample, pending = false, false
	if not ok then
		local reported_ok = pcall(report, "sample", tostring(err))
		if not reported_ok then
			disabled = true
			print("[GR2MovementRecorder] DISABLED: " .. tostring(err) .. "\n")
		end
	end
end

---@return boolean
local function poll()
	if disabled then
		return true
	end
	-- Coalescing avoids an unbounded queue while the game is paused/loading.
	if not pending then
		pending = true
		game_thread(scheduled_sample)
	end
	return false
end

emit({
	kind = "manifest",
	recorder = "GR2MovementRecorder/2.4",
	clocks = {
		world_time = "GameplayStatics.GetTimeSeconds(pawn)",
		world_audio_time = "GameplayStatics.GetAudioTimeSeconds(pawn)",
	},
	source_build = "16083277",
	expected_binary_sha256 = "3900626fe173ec8f22c963876137b207c85ba0911b82ba17696b9e5e4a49af85",
	started_utc = os.date("!%Y-%m-%dT%H:%M:%SZ"),
	settings = settings,
	sampling = "8ms scheduler request; coalesced ExecuteInGameThread; phase/tick unverified",
	units = "Unreal coordinates cm; velocity cm/s; world clock seconds; axes retained raw",
	limits = "Reflected events may miss direct native calls; no hardware input replay; unknown fields omitted; structs whitelist; arrays capped 256",
})
---@return nil
local function initialize()
	local enum = find_static("/Script/Ghostrunner.EUpgradeType")
	if valid(enum) and enum:type() == "UEnum" then
		---@cast enum CaptureEnum
		local names = {}
		---@param name CaptureName
		---@param value integer
		---@return nil
		local function enum_name(name, value)
			names[tostring(value)] = name:ToString()
		end
		enum:ForEachName(enum_name)
		emit({ kind = "upgrade_enum", path = enum:GetFullName(), available = true, names = names })
	else
		emit({ kind = "upgrade_enum", path = "/Script/Ghostrunner.EUpgradeType", available = false })
	end
	clock = find_static("/Script/Engine.Default__GameplayStatics")
	if not clock:IsValid() then
		clock = nil
		report("clock", "GameplayStatics unavailable; world timing unknown")
	end
end
game_thread(initialize)
print("[GR2MovementRecorder] loaded; traces=" .. settings.output_directory .. "; waiting for gameplay pawn\n")
---@return boolean
local function drain()
	local ok, err = pcall(writer.drain, writer)
	if not ok then
		disabled = true
		print("[GR2MovementRecorder] writer failed: " .. tostring(err) .. "\n")
		return true
	end
	return false
end
loop_async(settings.writer_poll_ms, drain)
loop_async(settings.poll_ms, poll)
