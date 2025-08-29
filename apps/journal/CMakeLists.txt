cmake_minimum_required(VERSION 3.31)
set(CMAKE_EXPORT_COMPILE_COMMANDS ON)
project(panorama-journal LANGUAGES CXX)

set(CMAKE_MODULE_PATH "${CMAKE_CURRENT_SOURCE_DIR}/../../cmake" ${CMAKE_MODULE_PATH})
include(GetTriple)
set(HOST_TRIPLE "")
set(HOST_ARCH "")
set(HOST_VENDOR "")
set(HOST_OS "")
get_host_triple(HOST_TRIPLE HOST_ARCH HOST_VENDOR HOST_OS)

set(CMAKE_AUTOMOC ON)
set(CMAKE_AUTORCC ON)
set(CMAKE_AUTOUIC ON)

set(DIST_DIR "${CMAKE_CURRENT_SOURCE_DIR}/dist/${HOST_TRIPLE}")
file(MAKE_DIRECTORY ${DIST_DIR})

add_library(panorama-journal SHARED
    src/JournalPlugin.cpp
)

target_link_libraries(panorama-journal PRIVATE
    panorama-plugin-sdk
)

set_target_properties(panorama-journal PROPERTIES
    LIBRARY_OUTPUT_DIRECTORY "${DIST_DIR}"
    RUNTIME_OUTPUT_DIRECTORY "${DIST_DIR}"
)
