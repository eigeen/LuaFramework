local InputUtils = {}

---@param control_key integer KeyCode
---@param key integer KeyCode
function InputUtils.on_hotkey(control_key, key)
    local control_keys = nil
    if type(control_key) == "table" then
        control_keys = control_key
    else
        control_keys = {control_key}
    end

    for _, key in ipairs(control_keys) do
        if not sdk.Input.keyboard.is_down(key) then
            return false
        end
    end

    if sdk.Input.keyboard.is_pressed(key) then
        return true
    end
    return false
end

return InputUtils
