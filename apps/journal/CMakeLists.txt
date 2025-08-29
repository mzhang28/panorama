cmake_minimum_required(VERSION 3.31)
set(CMAKE_EXPORT_COMPILE_COMMANDS ON)
project(panorama-journal LANGUAGES CXX)

set(CMAKE_AUTOMOC ON)
set(CMAKE_AUTORCC ON)
set(CMAKE_AUTOUIC ON)

add_library(panorama-journal SHARED
    src/JournalPlugin.cpp
)

target_link_libraries(panorama-journal PRIVATE
    panorama-plugin-sdk
)
