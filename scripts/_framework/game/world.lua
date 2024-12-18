local Structure = require("_framework.structure")
local Player = require("_framework.game.player")

local sMhAI = sdk.get_singleton("sMhAI")

---@class World
local World = {}

---@return World
function World.default()
    local world_data = sMhAI:offset(0x90, 0x40, 0x90, 0x18):read_ptr()
    local map = Player.get_master_player().map_data

    local obj = {
        navigation_position = {
            _ptr = world_data,
            x = Structure.field_def(0x200, "f32"),
            y = Structure.field_def(0x204, "f32"),
            z = Structure.field_def(0x208, "f32")
        }
    }

    local obj = Structure.new_nested(obj)

    return obj
end

return World
