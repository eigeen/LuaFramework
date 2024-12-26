local InputUtils = {}

local function is_down(key, is_controller)
    if is_controller then
        return sdk.Input.controller.is_down(key)
    else
        return sdk.Input.keyboard.is_down(key)
    end
end

local function is_pressed(key, is_controller)
    if is_controller then
        return sdk.Input.controller.is_pressed(key)
    else
        return sdk.Input.keyboard.is_pressed(key)
    end
end

local function generate_keyid(keys, is_controller, extra_id)
    local key_id = nil
    if is_controller then
        key_id = "controller_"
    else
        key_id = "key_"
    end
    for _, key in ipairs(keys) do
        key_id = key_id .. tostring(key)
    end
    if extra_id ~= nil then
        key_id = key_id .. "_" .. tostring(extra_id)
    end

    return key_id
end

---@param control_key integer | integer[] @ one or more KeyCode
---@param key integer KeyCode
---@param is_controller boolean? @ default: false, whether to use controller or keyboard
---@return boolean
function InputUtils.hotkey(control_key, key, is_controller)
    if is_controller == nil then
        is_controller = false
    end

    local control_keys = nil
    if type(control_key) == "table" then
        control_keys = control_key
    else
        control_keys = {control_key}
    end

    for _, key in ipairs(control_keys) do
        if not is_down(key, is_controller) then
            return false
        end
    end

    if is_pressed(key, is_controller) then
        return true
    end
    return false
end

-- keyid -> timer
-- if timer == true, means key is holding and has triggered the hold event.
local g_hold_timers = {}

---Hold keys for a certain duration.
---The key event will only be triggered once, like on_pressed.
---@param keys integer | integer[] @ KeyCode or KeyCode[]
---@param duration number @ time in milliseconds
---@param is_controller boolean? @ default: false, whether to use controller or keyboard
---@param extra_id string? @ extra id for keyid, used to distinguish different hold events
---@return boolean
function InputUtils.hold_keys(key, duration, is_controller, extra_id)
    local keys = nil
    if type(key) ~= "table" then
        keys = {key}
    else
        keys = key
    end
    if is_controller == nil then
        is_controller = false
    end

    local is_holding = true
    for _, keycode in ipairs(keys) do
        if not is_down(keycode, is_controller) then
            is_holding = false
            break
        end
    end

    local key_id = generate_keyid(keys, is_controller, extra_id)

    if not is_holding then
        g_hold_timers[key_id] = nil
        return false
    end

    local timer = g_hold_timers[key_id]
    -- if don't need to use timer
    if duration == nil or duration <= 0 then
        if timer == nil then
            g_hold_timers[key_id] = true
            return true
        end
        return false
    end

    if timer == nil then
        g_hold_timers[key_id] = utils.Instant.now()
        return false
    elseif timer == true then
        -- already triggered hold event
        return false
    end

    if timer:elapsed():as_millis() >= duration then
        g_hold_timers[key_id] = true
        return true
    end

    return false
end

-- record the last time of click event
-- used to check double click event
-- keyid -> timer
local g_click_timers = {}

---Double click keys in a certain duration.
---@param control_key integer | integer[] @ KeyCode or KeyCode[]
---@param key integer @ KeyCode
---@param duration number @ time in milliseconds
---@param is_controller boolean? @ default: false, whether to use controller or keyboard
---@param extra_id string? @ extra id for keyid, used to distinguish different events
---@return boolean
function InputUtils.double_click(control_key, key, duration, is_controller, extra_id)
    if is_controller == nil then
        is_controller = false
    end

    local control_keys = nil
    if type(control_key) == "table" then
        control_keys = control_key
    else
        control_keys = {control_key}
    end

    for _, key in ipairs(control_keys) do
        if not is_down(key, is_controller) then
            return false
        end
    end

    local keys_for_id = {}
    for _, key in ipairs(control_keys) do
        table.insert(keys_for_id, key)
    end
    table.insert(keys_for_id, key)
    local key_id = generate_keyid(keys_for_id, is_controller, extra_id)

    if is_pressed(key, is_controller) then
        local timer = g_click_timers[key_id]
        if timer == nil or timer:elapsed():as_millis() >= duration then
            g_click_timers[key_id] = utils.Instant.now()
            return false
        end

        g_click_timers[key_id] = nil
        return true
    end
    return false
end

return InputUtils
