local ControllerButton = {
    Share = 0x0001,
    L3 = 0x0002,
    R3 = 0x0004,
    Options = 0x0008,
    Up = 0x0010,
    Right = 0x0020,
    Down = 0x0040,
    Left = 0x0080,
    L1 = 0x0100,
    R1 = 0x0200,
    L2 = 0x0400,
    R2 = 0x0800,
    Triangle = 0x1000,
    Circle = 0x2000,
    Cross = 0x4000,
    Square = 0x8000,
    LsUp = 0x10000,
    LsRight = 0x20000,
    LsDown = 0x40000,
    LsLeft = 0x80000,
    RsUp = 0x100000,
    RsRight = 0x200000,
    RsDown = 0x400000,
    RsLeft = 0x800000
}

setmetatable(ControllerButton, {
    __newindex = function(table, key, value)
        error("Attempt to modify a read-only enum table 'ControllerButton'") -- 禁止修改
    end
})

return ControllerButton
