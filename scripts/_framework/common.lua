local g_bgm_manager = sdk.get_singleton("sWwiseBgmManager")

---@class CommonModule
local CommonModule = {}
CommonModule.__index = CommonModule

---玩家是否在场景中
---@return boolean
function CommonModule.is_player_in_scene()
    if g_bgm_manager:offset(0x50):read_i64() == 0 then
        return false
    end
    return true
end

return CommonModule
