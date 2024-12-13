import shutil
import os

os.system("cargo build --release --package lua-framework")
os.system("cargo build --release --package luaf-libffi")

shutil.copy(
    "target/release/lua_framework.dll",
    "C:/Program Files (x86)/Steam/steamapps/common/Monster Hunter World/nativePC/plugins/lua_framework.dll",
)
shutil.copy(
    "target/release/luaf_libffi.dll",
    "C:/Program Files (x86)/Steam/steamapps/common/Monster Hunter World/lua_framework/extensions/luaf_libffi.dll",
)
