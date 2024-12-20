--- Structure manager metatable
--
---@class FieldDef
---@field offset integer
---@field type string
local _ = _

--- Get field value from FieldDef
---@param ptr LuaPtr
---@param def FieldDef
local function get_field(ptr, def)
    local offset = def.offset
    local ty = def.type

    local offset_ptr = ptr:offset(offset)

    if ty == "u8" then
        return offset_ptr:read_u8()
    elseif ty == "i8" then
        return offset_ptr:read_i8()
    elseif ty == "u16" then
        return offset_ptr:read_u16()
    elseif ty == "i16" then
        return offset_ptr:read_i16()
    elseif ty == "u32" then
        return offset_ptr:read_u32()
    elseif ty == "i32" then
        return offset_ptr:read_i32()
    elseif ty == "u64" then
        return offset_ptr:read_u64()
    elseif ty == "i64" then
        return offset_ptr:read_i64()
    elseif ty == "f32" then
        return offset_ptr:read_f32()
    elseif ty == "f64" then
        return offset_ptr:read_f64()
    elseif ty == "string" then
        local str_ptr = offset_ptr:read_ptr()
        return sdk.String.from_ptr(str_ptr)
    elseif ty == "pointer" then
        return offset_ptr:read_ptr()
    end

    error("Unsupported field type: " .. ty)
end

--- Set field value from FieldDef
---@field ptr LuaPtr
---@field def FieldDef
---@field value any
local function set_field(ptr, def, value)
    local offset = def.offset
    local ty = def.type

    local offset_ptr = ptr:offset(offset)

    if ty == "u8" then
        offset_ptr:write_u8(value)
    elseif ty == "i8" then
        offset_ptr:write_i8(value)
    elseif ty == "u16" then
        offset_ptr:write_u16(value)
    elseif ty == "i16" then
        offset_ptr:write_i16(value)
    elseif ty == "u32" then
        offset_ptr:write_u32(value)
    elseif ty == "i32" then
        offset_ptr:write_i32(value)
    elseif ty == "u64" then
        offset_ptr:write_u64(value)
    elseif ty == "i64" then
        offset_ptr:write_i64(value)
    elseif ty == "f32" then
        offset_ptr:write_f32(value)
    elseif ty == "f64" then
        offset_ptr:write_f64(value)
    elseif ty == "string" then
        -- assert if value is ManagedString
        assert(type(value) == "userdata" and value.len)

        local str_bytes = value:to_bytes_with_nul()
        offset_ptr:write_bytes(str_bytes)
    elseif ty == "pointer" then
        error("Modifying pointer field is not allowed currently.")
    else
        error("Unsupported field type: " .. ty)
    end
end

local function is_field_def(tbl)
    if tbl["offset"] and tbl["type"] then
        return true
    end
    return false
end

-- for cross-reference, predefine here
local StructureMeta = nil
local Structure = nil

StructureMeta = {
    __index = function(tbl, key)
        local raw_tbl = rawget(tbl, "_record")
        local value = rawget(raw_tbl, key)
        if value == nil then
            return nil
        end

        local value_ty = type(value)
        if value_ty ~= "table" then
            return value
        end

        local _ptr = rawget(raw_tbl, "_ptr")
        if is_field_def(value) then
            -- field def
            if value._ptr then
                _ptr = value._ptr
            end
            return get_field(_ptr, value)
        end

        -- nested structure
        local value_raw_tbl = rawget(value, "_record")
        if not value_raw_tbl._ptr then
            rawset(value_raw_tbl, "_ptr", _ptr) -- transfer _ptr
        end
        return value
    end,
    __newindex = function(tbl, key, value)
        local raw_tbl = rawget(tbl, "_record")
        local tbl_value = rawget(raw_tbl, key)
        if tbl_value == nil then
            rawset(raw_tbl, key, value)
            return
        end

        local tbl_value_ty = type(tbl_value)
        if tbl_value_ty ~= "table" then
            rawset(raw_tbl, key, value)
            return
        end

        local _ptr = rawget(raw_tbl, "_ptr")
        if is_field_def(tbl_value) then
            -- field def
            if tbl_value._ptr then
                _ptr = tbl_value._ptr
            end
            set_field(_ptr, tbl_value, value)
            return
        end

        -- try to process nested field set
        if type(value) == "table" then
            for k, v in pairs(value) do
                local value_raw_tbl = rawget(tbl_value, "_record")
                if not value_raw_tbl._ptr then
                    rawset(value_raw_tbl, "_ptr", _ptr) -- transfer _ptr
                end
                tbl_value[k] = v
            end
            return
        end
    end
}

Structure = {}

function Structure.new(tbl)
    -- auto store raw table in _record field
    -- user should store pointer in _ptr field
    local result = {}
    result._record = tbl

    local mt = getmetatable(tbl)
    if mt == nil then
        setmetatable(result, StructureMeta)
        return result
    else
        mt.__index = StructureMeta.__index
        mt.__newindex = StructureMeta.__newindex
        setmetatable(result, mt)
        return result
    end
end

function Structure.new_nested(tbl)
    for k, v in pairs(tbl) do
        if type(v) ~= "table" then
            goto continue
        end
        if is_field_def(v) then
            goto continue
        end

        tbl[k] = Structure.new_nested(v)
        ::continue::
    end

    return Structure.new(tbl)
end

function Structure.field_def(offset, ty, ptr)
    return {
        offset = offset,
        type = ty,
        _ptr = ptr
    }
end

---Get nested field value from definition table
---@param tbl table
---@return table
function Structure.get_nested_field(tbl)
    local result = {}

    for key in pairs(tbl._record) do
        local value = tbl[key]
        if type(value) == "table" then
            print(string.format("key: %s, value: %s", tostring(key), FormatPretty.table(value, {
                show_metatable = true
            })))
        else
            print(string.format("key: %s, value: %s", tostring(key), tostring(value)))
        end
        if key == "_ptr" then
            result[key] = value
            goto continue_nested_field
        end

        if type(value) == "table" then
            local nested = Structure.get_nested_field(value)
            if nested then
                result[key] = nested
                goto continue_nested_field
            end
        end

        result[key] = value

        ::continue_nested_field::
    end

    return result
end

return Structure
