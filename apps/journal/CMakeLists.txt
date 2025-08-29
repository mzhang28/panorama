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

#---------------------------------------------------------------------------------------------------------
# Dependencies

find_package(Qt6 REQUIRED COMPONENTS Core Widgets Network)

include(FetchContent)

set(QMARKDOWNTEXTEDIT_EXE OFF CACHE BOOL "Do not build qmarkdowntextedit executable" FORCE)
set(QMARKDOWNTEXTEDIT_QTQUICK_EXAMPLES OFF CACHE BOOL "Do not build QtQuickExample" FORCE)
FetchContent_Declare(
    QMarkdownTextEdit
    GIT_REPOSITORY https://github.com/pbek/qmarkdowntextedit.git
    GIT_TAG 0ada3f3a156fdadd92c91368684ef0f406b835e2 # some random commit i fetched off main, this project doesn't use tags...
)
FetchContent_MakeAvailable(QMarkdownTextEdit)

#---------------------------------------------------------------------------------------------------------
# Library

set(DIST_DIR "${CMAKE_CURRENT_SOURCE_DIR}/dist/${HOST_TRIPLE}")
file(MAKE_DIRECTORY ${DIST_DIR})

add_library(panorama-journal SHARED
    src/Journal.cpp
    src/JournalPlugin.cpp
    src/JournalStore.cpp
    src/MarkdownEdit.cpp
)

target_include_directories(panorama-journal PRIVATE
    "${qmarkdowntextedit_SOURCE_DIR}"
)

target_link_libraries(panorama-journal PRIVATE
    panorama-plugin-sdk
    qmarkdowntextedit
    Qt6::Core
    Qt6::Network
    Qt6::Widgets
)

set_target_properties(panorama-journal PROPERTIES
    LIBRARY_OUTPUT_DIRECTORY "${DIST_DIR}"
    RUNTIME_OUTPUT_DIRECTORY "${DIST_DIR}"
)
