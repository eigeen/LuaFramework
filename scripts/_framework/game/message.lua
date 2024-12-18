local Common = require("_framework.game.common")

local g_chat = sdk.get_singleton("sChat")
local g_system_send = sdk.AddressRepository.get_or_insert("Chat:SystemMessage",
    "0F 29 B4 24 B0 01 00 00 48 8B DA 0F 28 F2 48 8B F9 75 09", -25)

---@class MessageModule
local MessageModule = {}
MessageModule.__index = MessageModule

---显示系统消息（聊天栏）
---@param message string
---@param color integer? MessageModule.SystemMessageColor
function MessageModule.show_system(message, color)
    if color == nil then
        color = MessageModule.SystemMessageColor.Blue
    end

    if not Common.is_player_in_scene() then
        return
    end

    -- extern "C" fn(*const c_void, *const i8, i32, i32, i8)
    local msg = sdk.String.new_utf8(message)
    sdk.call_native_function(g_system_send, {{
        type = "pointer",
        value = g_chat:to_integer()
    }, {
        type = "string",
        value = msg
    }, {
        type = "i32",
        value = msg:len()
    }, {
        type = "i32",
        value = -1
    }, {
        type = "i8",
        value = color
    }})
end

MessageModule.SystemMessageColor = {
    Blue = 0,
    Purple = 1
}

return MessageModule
