local Structure = require("_framework.structure")

local sWwiseBgmManager = sdk.get_singleton("sWwiseBgmManager")
local sSetObject = sdk.get_singleton("sSetObject")

---@class Player
local Player = {}

---@return Player
function Player.get_master_player()
    local obj = {
        -- 坐标
        position = {
            x = Structure.field_def(0x160, "f32"),
            y = Structure.field_def(0x164, "f32"),
            z = Structure.field_def(0x168, "f32")
        },
        -- 遣返坐标
        reposition = {
            x = Structure.field_def(0xA50, "f32"),
            y = Structure.field_def(0xA54, "f32"),
            z = Structure.field_def(0xA58, "f32")
        },
        cntrposition = {},
        incremental = {},
        action = {
            lmt_id = Structure.field_def({0x468, 0xE9C4}, "i32"),
            fsm = {
                target = Structure.field_def(0x6274, "i32"),
                id = Structure.field_def(0x6278, "i32")
            }
        },
        frame_speed_multiplier = {
            get = function(_ptr)
                local a = sSetObject:offset(0x78, 0x10, 0x80)
                local b = _ptr:offset(0x10):read_i32()
                local mul_ptr = sdk.LuaPtr(a:to_integer() + 0xF8 * b + 0x9C)
                return mul_ptr:read_f32()
            end,
            set = function(_ptr, value)
                local a = sSetObject:offset(0x78, 0x10, 0x80)
                local b = _ptr:offset(0x10):read_i32()
                local mul_ptr = sdk.LuaPtr(a:to_integer() + 0xF8 * b + 0x9C)
                mul_ptr:write_f32(value)
            end
        },
        map_data = Structure.field_def(0x7D20, "pointer")
    }

    local ptr_player = sWwiseBgmManager:offset(0x50):read_ptr()
    obj._ptr = ptr_player

    local obj = Structure.new_nested(obj)

    return obj
end

return Player
