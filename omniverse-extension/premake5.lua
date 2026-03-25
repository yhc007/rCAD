-- Premake5 configuration for rCAD Omniverse Extension

workspace "rcad-omniverse"
    configurations { "Debug", "Release" }
    platforms { "x64" }
    location "build"

project "rcad.connector"
    kind "SharedLib"
    language "C++"
    cppdialect "C++17"

    targetdir "bin/%{cfg.buildcfg}"
    objdir "obj/%{cfg.buildcfg}"

    files {
        "src/**.h",
        "src/**.cpp"
    }

    includedirs {
        "include",
        -- Omniverse SDK paths would go here
    }

    filter "configurations:Debug"
        defines { "DEBUG" }
        symbols "On"

    filter "configurations:Release"
        defines { "NDEBUG" }
        optimize "On"

    filter "platforms:x64"
        architecture "x86_64"
