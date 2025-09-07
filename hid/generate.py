import os

template = r"""#pragma comment(linker, "/export:{0}=\"C:\\Windows\\System32\\{1}.{0}\"")"""

module_name = "hid"

input_file_path = os.path.dirname(__file__) + f"\\{module_name}.dll.ExportFunctions.txt"

with open(input_file_path, "r") as f:
    for line in f:
        parts = line.strip().split("\t")
        if parts[0] == "Ordinal":
            continue
        if len(parts) < 4:
            continue
        print(template.format(parts[3], module_name))