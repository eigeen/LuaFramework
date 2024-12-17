import os
import shutil
import zipfile
import re

# create dist folder
if os.path.exists("dist"):
    shutil.rmtree("dist")

os.makedirs("dist")

# run rust build command
os.system("cargo build --release --package lua-framework --package luaf-libffi")

file_src_dst = [
    {
        "type": "file",
        "src": "target/release/lua_framework.dll",
        "dst": "nativePC/plugins/lua_framework.dll",
    },
    {
        "type": "file",
        "src": "target/release/luaf_libffi.dll",
        "dst": "lua_framework/extensions/luaf_libffi.dll",
    },
    {"type": "create_dir", "dst": "lua_framework/scripts"},
    {
        "type": "dir",
        "src": "scripts/_framework",
        "dst": "lua_framework/scripts/_framework",
    },
]

for src_dst in file_src_dst:
    if src_dst["type"] == "file":
        dst = "./dist/" + src_dst["dst"]
        os.makedirs(os.path.dirname(dst), exist_ok=True)
        shutil.copy(src_dst["src"], dst)
    elif src_dst["type"] == "dir":
        shutil.copytree(src_dst["src"], "./dist/" + src_dst["dst"])
    elif src_dst["type"] == "create_dir":
        os.makedirs("./dist/" + src_dst["dst"])

# get version from Cargo.toml
version = ""
with open("Cargo.toml", "r", encoding="utf-8") as f:
    for line in f:
        if line.startswith("version"):
            results = re.findall(r'version = "(\d+\.\d+\.\d+)"', line)
            if len(results) > 0:
                version = results[0]

# create zip file
archive = zipfile.ZipFile(
    f"dist/lua-framework_{version}.zip", "w", zipfile.ZIP_DEFLATED
)

for src_dst in file_src_dst:
    if src_dst["type"] == "file":
        archive.write(src_dst["src"], src_dst["dst"])
    elif src_dst["type"] == "dir":
        for root, dirs, files in os.walk(src_dst["src"]):
            for file in files:
                archive.write(
                    os.path.join(root, file),
                    os.path.join(
                        src_dst["dst"],
                        os.path.relpath(os.path.join(root, file), src_dst["src"]),
                    ),
                )
    elif src_dst["type"] == "create_dir":
        archive.mkdir(src_dst["dst"])

archive.close()
