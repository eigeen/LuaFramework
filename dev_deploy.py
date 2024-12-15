import shutil
import os

os.system("cargo build --release --package lua-framework --package luaf-libffi")

shutil.copy(
    "target/release/lua_framework.dll",
    "C:/Program Files (x86)/Steam/steamapps/common/Monster Hunter World/nativePC/plugins/lua_framework.dll",
)
shutil.copy(
    "target/release/luaf_libffi.dll",
    "C:/Program Files (x86)/Steam/steamapps/common/Monster Hunter World/lua_framework/extensions/luaf_libffi.dll",
)
shutil.copytree("scripts/_framework", "C:/Program Files (x86)/Steam/steamapps/common/Monster Hunter World/lua_framework/scripts/_framework")
