math.randomseed(os.time())

---@class Mutex
local Mutex = {}

---Create a new Mutex object.
function Mutex.new(label)
    local obj = {
        id = math.random(), -- mark the locker
        label = label, -- label of the protected resource
        owning = false, -- true if the owner is self
    }
    setmetatable(obj, {
        __index = Mutex
    })
    return obj
end

function Mutex:get_lock_state_key()
    return "mutex_" .. tostring(self.label)
end

function Mutex:is_locked()
    local locker = sdk.ShardState.get(self:get_lock_state_key())
    return locker ~= nil
end

function Mutex:is_owning_lock()
    local locker = sdk.ShardState.get(self:get_lock_state_key())
    return locker ~= nil
end

---@return boolean
function Mutex:lock()
    -- Check if the mutex is already locked
    local locker = sdk.ShardState.get(self:get_lock_state_key())
    if locker then
        return false
    else
        sdk.ShardState.set("mutex_" .. tostring(self.label), self.id)
        self.owning = true
        return true
    end
end

---@return boolean
function Mutex:unlock()
    local locker = sdk.ShardState.get(self:get_lock_state_key())
    if locker and locker == self.id then
        sdk.ShardState.set("mutex_" .. tostring(self.label), nil)
        self.owning = false
        return true
    end
    return false
end

local LockAPI = {}

---@param label any
---@return Mutex
function LockAPI.Mutex(label)
    return Mutex.new(label)
end

return LockAPI
