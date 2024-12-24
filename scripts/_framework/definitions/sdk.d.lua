---@class sdk
---@field Input Input
---@field String _TStringConstructor
---@field LuaPtr _TLuaPtrConstructor
---@field Memory Memory
---@field AddressRepository AddressRepository
---@field Interceptor Interceptor

local sdk = {
    ---@class _TStringConstructor
    ---@field new_utf8 fun(str:string): ManagedString
    ---@field new_utf16 fun(str:string): ManagedString
    ---@field from_ptr fun(ptr:AsLuaPtr): ManagedString
    ---@field from_utf8_bytes fun(bytes:Bytes): ManagedString
    String = {},
    ---@class _TLuaPtrConstructor
    ---@field __call fun(address:integer): LuaPtr
    LuaPtr = {},
}

-- typedef
---@alias Bytes table<integer, integer>
---@alias UInt64 table<string, integer> @ 暂时未被广泛使用，仅占位。LuaTable实现的长整型，适用于FFI安全调用。
---@alias AsLuaPtr LuaPtr @ 指示该值能够被转换为LuaPtr的类型。具体参考LuaPtr创建函数。

---@class Input
---@field keyboard _Tkey
---@field controller _Tcontroller
local Input = {
    ---@class _Tkey
    ---@field is_down fun():boolean
    ---@field is_pressed fun():boolean
    keyboard = {},
    ---@class _Tcontroller
    ---@field is_down fun():boolean
    ---@field is_pressed fun():boolean
    controller = {}
}

---@class ManagedString
---@field encoding string "utf8" | "utf16"
---@field len fun():integer
---@field to_string fun():string
---@field to_bytes_with_nul fun():Bytes
---@field to_bytes fun():Bytes

---@class LuaPtr
---@field to_integer fun():integer
---@field to_uint64 fun():UInt64
---@field read_integer fun(size:integer): integer
---@field read_bytes fun(size:integer): Bytes
---@field write_integer fun(value:integer, size:integer)
---@field write_bytes fun(value:Bytes, size:integer|nil)
---@field read_u8 fun(): integer
---@field read_i8 fun(): integer
---@field read_u16 fun(): integer
---@field read_i16 fun(): integer
---@field read_u32 fun(): integer
---@field read_i32 fun(): integer
---@field read_u64 fun(): integer
---@field read_i64 fun(): integer
---@field write_u8 fun(value:integer)
---@field write_i8 fun(value:integer)
---@field write_u16 fun(value:integer)
---@field write_i16 fun(value:integer)
---@field write_u32 fun(value:integer)
---@field write_i32 fun(value:integer)
---@field write_u64 fun(value:integer)
---@field write_i64 fun(value:integer)
---@field read_f32 fun(): number
---@field read_f64 fun(): number
---@field write_f32 fun(value:number)
---@field write_f64 fun(value:number)
---@field read_ptr fun(): LuaPtr @ 读取当前指针的值，并将新的值作为 LuaPtr 返回。
---@field offset fun(...): LuaPtr @ 偏移指针。支持传入多个变量进行多级偏移。返回新的LuaPtr，可链式调用。
---@field offset_ce fun(...): LuaPtr @ CE方法偏移指针。支持传入多个变量进行多级偏移。与默认方法相比，该方法会先对基址进行取值操作。等效于 `:read_ptr():offset()`。返回新的LuaPtr，可链式调用。

---@class Memory
---@field scan fun(address:integer, size:integer, pattern:string, offset:integer|nil): LuaPtr
---@field scan_all fun(address:integer, size:integer, pattern:string, offset:integer|nil): table<integer, LuaPtr>

---@class AddressRepository
---@field get fun(name:string): LuaPtr
---@field try_get fun(name:string): table<nil, nil> @ return: (ok: boolean, ptr_or_error: LuaPtr|string)
---@field set_record fun() @ 接受 AddressRecord 或 (name:string, pattern:string, offset:integer|nil)
---@field get_or_insert fun(): LuaPtr @ 接受 AddressRecord 或 (name:string, pattern:string, offset:integer|nil)。尝试获取已记录的特征码地址，若不存在则插入新记录并获取值。

---@class Interceptor
---@field attach fun()
---@field attach_instruction fun()
---@field detach fun()
