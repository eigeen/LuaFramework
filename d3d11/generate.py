import os

template = r"""#pragma comment(linker, "/export:{0}=\"C:\\Windows\\System32\\{1}.{0}\"")"""

module_name = "d3d11"

input_file_path = os.path.dirname(__file__) + "\\d3d11.dll.ExportFunctions.txt"

with open(input_file_path, "r") as f:
    for line in f:
        parts = line.strip().split("\t")
        if parts[0] == "Ordinal":
            continue
        print(template.format(parts[3], module_name))