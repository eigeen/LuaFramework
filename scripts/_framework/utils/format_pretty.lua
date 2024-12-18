--- Format a table as a string prettier.
---@author: Eigeen
---@date: 2024/12/05

local function concat_default_config(config)
    local default = {
        pretty = false,
        multiline = false,
        indent = 4,
        show_metatable = false
    }
    if not config then
        return default
    end

    local new = {}
    for k, v in pairs(default) do
        if config[k] == nil then
            new[k] = v
        else
            new[k] = config[k]
        end
    end
    return new
end

local function table_contains(table, value)
    for _, v in pairs(table) do
        if v == value then
            return true
        end
    end
    return false
end

local function create_indent(depth, indent_size)
    return string.rep(" ", depth * indent_size)
end

local function format_table_pretty_inner(key, tbl, config, visited, depth)
    local visited_tbl = visited[tbl]
    if visited_tbl then
        return "<ref: " .. tostring(visited_tbl.first_key) .. ">"
    end
    visited[tbl] = {
        first_key = key
    }

    local field_strs = {}

    for key, value in pairs(tbl) do
        local value_str = "unk"
        if type(value) == "table" then
            -- 递归处理
            value_str = format_table_pretty_inner(key, value, config, visited, depth + 1)
        elseif type(value) == "string" then
            value_str = "\"" .. value .. "\""
        elseif table_contains({"function", "userdata", "thread"}, type(value)) then
            value_str = "<" .. tostring(value) .. ">"
        else -- 其他类型
            value_str = tostring(value)
        end

        table.insert(field_strs, string.format("%s = %s", tostring(key), value_str))
    end

    local metatable_str = nil
    if config.show_metatable then
        local mt = getmetatable(tbl)
        if mt then
            -- 递归处理
            metatable_str = format_table_pretty_inner("metatable", mt, config, visited, depth + 1)
        end
    end

    -- 处理tokens，格式化输出
    if config.multiline then 
        -- 多行模式
        local result = "{" -- 首行不缩进
        if #field_strs > 0 or metatable_str then
            result = result .. "\n" .. create_indent(depth + 1, config.indent)
        end
    
        result = result .. table.concat(field_strs, ",\n" .. create_indent(depth + 1, config.indent))
    
        if metatable_str then
            if #field_strs > 0 then
                result = result .. ",\n" .. create_indent(depth + 1, config.indent)
            end
            result = result .. "<metatable> = " .. metatable_str
        end
        result = result .. "\n" .. create_indent(depth, config.indent) .. "}"
    
        return result
    else 
        -- 单行模式
        local result = "{ "
        result = result .. table.concat(field_strs, ", ")
        if metatable_str then
            if #field_strs > 0 then
                result = result .. ", "
            end
            result = result .. "<metatable> = " .. metatable_str
        end
        result = result .. " }"
    
        return result
    end

end

---@class FormatPretty Formatter for Lua Table.
local FormatPretty = {}
FormatPretty.__index = FormatPretty

--- Format a table as a string.
--- 
--- Example output: `{ a = 1, b = "hello", 42 = true }`
---@param table table
---@return string
function FormatPretty.table(tbl, config)
    config = concat_default_config(config)

    local visited = {}

    return format_table_pretty_inner("_root", tbl, config, visited, 0)
end

--- Format a table as a string, prettier format (multiline).
--- An alias for `FormatPretty.table({...}, { multiline = true })`.
---@param table table
---@return string
function FormatPretty.table_pretty(tbl, config)
    if config == nil then
        config = {}
    end
    config.multiline = true

    return FormatPretty.table(tbl, config)
end

return FormatPretty
