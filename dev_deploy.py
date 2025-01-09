import os
import pathlib
import shutil

game_root = pathlib.Path("C:/Program Files (x86)/Steam/steamapps/common/Monster Hunter World")

os.system("cd d3d11 && xmake build -y")
os.system("cargo build --release --package lua-framework --package luaf-libffi")

shutil.copy(
    "d3d11/build/windows/x64/release/d3d11.dll",
    game_root.joinpath("d3d11.dll"),
)
shutil.copy(
    "target/release/lua_framework.dll",
    game_root.joinpath("lua_framework.dll"),
)
shutil.copy(
    "target/release/luaf_libffi.dll",
    game_root.joinpath("lua_framework/extensions/luaf_libffi.dll"),
)
shutil.copy(
    "mhw-imgui-core/x64/Release/mhw-imgui-core.dll",
    game_root.joinpath("lua_framework/extensions/mhw-imgui-core.dll"),
)
