---@class utils
---@field Instant _TInstantConstructor
local _ = _

local utils = {
    ---@class _TInstantConstructor
    ---@field now fun(): Instant
    Instant = {}
}

---@class Instant
---@field elapsed fun(): Duration

---@class Duration
---@field as_secs fun(): integer
---@field as_secs_f64 fun(): number
---@field as_millis fun(): integer
---@field as_micros fun(): integer
---@field as_nanos fun(): integer
