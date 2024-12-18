local Structure = require("_framework.structure")

local sQuest = sdk.get_singleton("sQuest")
local sMhEffect = sdk.get_singleton("sMhEffect")

---@class Quest
local Quest = {}

function Quest.default()
    local obj = {
        _ptr = sQuest,
        current_id = Structure.field_def(0x4C, "i32"),
        selected_id = Structure.field_def({0x570, 0x130 + 0x58, 0x50, 0x408, 0x292C}, "i32", sMhEffect),
        state = Structure.field_def(0x38, "i32"),
        state2 = Structure.field_def(0x54, "i32")
    }

    return Structure.new_nested(obj)
end

return Quest