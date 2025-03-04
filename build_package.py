import os
import re
import shutil
import subprocess
import sys
import zipfile

g_dev_mode = False


def get_commit_id_short():
    try:
        commit_hash = (
            subprocess.check_output(
                ["git", "rev-parse", "--short", "HEAD"], stderr=subprocess.STDOUT
            )
            .strip()
            .decode("utf-8")
        )
        return commit_hash
    except subprocess.CalledProcessError as e:
        print(f"Error: {e.output.decode('utf-8')}")
        return None


if len(sys.argv) >= 2:
    if sys.argv[1] == "dev":
        g_dev_mode = True

# create dist folder
if os.path.exists("dist"):
    shutil.rmtree("dist")

os.makedirs("dist")

# run build command
os.system("cd d3d11 && xmake build -y")
os.system("cargo build --release --package lua-framework --package luaf-libffi")

file_src_dst = [
    # loader
    {
        "type": "file",
        "src": "d3d11/build/windows/x64/release/d3d11.dll",
        "dst": "d3d11.dll",
    },
    # core files
    {
        "type": "file",
        "src": "target/release/lua_framework.dll",
        "dst": "lua_framework.dll",
    },
    {
        "type": "file",
        "src": "lib/cimgui.dll",
        "dst": "lua_framework/bin/cimgui.dll",
    },
    {
        "type": "file",
        "src": "target/release/luaf_libffi.dll",
        "dst": "lua_framework/extensions/luaf_libffi.dll",
    },
    {
        "type": "file",
        "src": "mhw-imgui-core/x64/Release/mhw-imgui-core.dll",
        "dst": "lua_framework/extensions/mhw-imgui-core.dll",
    },
    # assets
    {
        "type": "file",
        "src": "assets/SourceHanSansCN-Regular.otf",
        "dst": "lua_framework/fonts/SourceHanSansCN-Regular.otf",
    },
    # scripts
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

zip_name = None
if g_dev_mode:
    commit_id_short = get_commit_id_short()
    zip_name = f"lua-framework_v{version}-dev-{commit_id_short}.zip"
else:
    zip_name = f"lua-framework_v{version}.zip"

# create zip file
archive = zipfile.ZipFile(f"dist/{zip_name}", "w", zipfile.ZIP_DEFLATED)

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
