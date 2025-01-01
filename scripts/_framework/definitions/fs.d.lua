---@class json
---@field encode fun(data:any): string
---@field decode fun(str:string): any
---@field dump fun(path:string, data:any)
---@field dump_pretty fun(path:string, data:any)
---@field load fun(path:string): any
local _ = _

---@class toml
---@field encode fun(data:any): string
---@field decode fun(str:string): any
---@field dump fun(path:string, data:any)
---@field dump_pretty fun(path:string, data:any)
---@field load fun(path:string): any
