local TUtils = {}

function TUtils.deep_copy(original)
    if type(original) ~= 'table' then
        return original
    end

    local copy = {} 
    for key, value in next, original, nil do
        copy[TUtils.deep_copy(key)] = TUtils.deep_copy(value)
    end
    setmetatable(copy, TUtils.deep_copy(getmetatable(original)))
    return copy
end

function TUtils.array_contains(tbl, value)
    for _, v in ipairs(tbl) do
        if v == value then
            return true
        end
    end
    return false
end

function TUtils.contains(tbl, value)
    for _, v in pairs(tbl) do
        if v == value then
            return true
        end
    end
    return false
end

return TUtils
