set_project("hid_forward")

add_requires("wil")

target("hid")
    set_kind("shared")
    set_languages("c++17")

    add_links("user32")
    add_packages("wil")

    add_files("dllmain.cpp")
