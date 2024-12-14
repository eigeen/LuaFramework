---@class UInt64 安全长整型
local UInt64 = {
    high = 0,
    low = 0
}

local UInt64_MT = {
    __index = UInt64,
    __name = "UInt64",
    __add = function(a, b)
    end,
    __sub = function(a, b)
    end,
    __tostring = function(a)
    end,
    __call = function(t, ...)
        local args = {...}
        if #args == 0 then
            error("UInt64 requires at least one argument")
        end
        if #args == 1 then
            return UInt64.new(args[1])
        end
        if #args == 2 then
            return UInt64.new_raw(args[1], args[2])
        end
        error("UInt64 requires at most two arguments")
    end
}

setmetatable(UInt64, UInt64_MT)

function UInt64.new(value)
    if type(value) == "string" then
        local high, low = utils.parse_string_to_2u32(value)
        return UInt64.new_raw(low, high)
    elseif type(value) == "number" or type(value) == "integer" then
        -- number不可超过u32::MAX, integer不可超过i64::MAX
        local is_safe = utils.check_safe_to_ptr(value)
        if is_safe then
            local high, low = utils.parse_string_to_2u32(value)
            return UInt64.new_raw(low, high)
        else
            error("Number out of safe range to convert to UInt64:", value)
        end
    end

    error("Invalid UInt64 new argument:", value)
end

function UInt64.new_raw(high, low)
    local obj = {
        high = high,
        low = low,
        _type = "UInt64"
    }
    setmetatable(obj, UInt64_MT)
    return obj
end

_G.UInt64 = UInt64
