set_project("d3d11_forward")


add_requires("wil")

target("d3d11")
    set_kind("shared")
    set_languages("c++17")

    add_links("user32")
    add_packages("wil")

    add_files("dllmain.cpp")
