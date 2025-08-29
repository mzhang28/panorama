cmake_minimum_required(VERSION 3.31)
set(CMAKE_EXPORT_COMPILE_COMMANDS ON)
project(PanoramaUi LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(Qt6Core_USE_LIBINTL OFF)

if(APPLE)
    set(CMAKE_OSX_DEPLOYMENT_TARGET "15.2" CACHE STRING "Minimum OS X deployment version")
    message(STATUS "macOS detected, disabling Vulkan")
    set(Qt6_QT_FEATURE_vulkan OFF CACHE BOOL "" FORCE)
    set(CMAKE_DISABLE_FIND_PACKAGE_Vulkan ON)
endif()

if(CMAKE_BUILD_TYPE STREQUAL "Debug")
    set(CMAKE_CXX_FLAGS_DEBUG "${CMAKE_CXX_FLAGS_DEBUG} -fsanitize=address -fno-omit-frame-pointer -g")
    set(CMAKE_LINKER_FLAGS_DEBUG "${CMAKE_LINKER_FLAGS_DEBUG} -fsanitize=address")
endif()

#---------------------------------------------------------------------------------------------------------
# Dependencies

find_package(Qt6 REQUIRED COMPONENTS Core Widgets Network Pdf PdfWidgets WebSockets)

set(CMAKE_SUPPRESS_DEVELOPER_WARNINGS 1)

include(FetchContent)

FetchContent_Declare(
    ADS
    GIT_REPOSITORY https://github.com/githubuser0xFFFF/Qt-Advanced-Docking-System.git
    GIT_TAG        4.4.0
)
FetchContent_MakeAvailable(ADS)

set(QMARKDOWNTEXTEDIT_EXE OFF CACHE BOOL "Do not build qmarkdowntextedit executable" FORCE)
set(QMARKDOWNTEXTEDIT_QTQUICK_EXAMPLES OFF CACHE BOOL "Do not build QtQuickExample" FORCE)
FetchContent_Declare(
    QMarkdownTextEdit
    GIT_REPOSITORY https://github.com/pbek/qmarkdowntextedit.git
    GIT_TAG 0ada3f3a156fdadd92c91368684ef0f406b835e2 # some random commit i fetched off main, this project doesn't use tags...
)
FetchContent_MakeAvailable(QMarkdownTextEdit)

set(CMAKE_SUPPRESS_DEVELOPER_WARNINGS 0)

#---------------------------------------------------------------------------------------------------------
# paanorama-core

add_subdirectory(panorama-core)
if(CMAKE_BUILD_TYPE STREQUAL "Debug")
    set(RUST_BUILD_TYPE "debug")
else()
    set(RUST_BUILD_TYPE "release")
endif()

set(CMAKE_AUTOMOC ON)
set(CMAKE_AUTORCC ON)
set(CMAKE_AUTOUIC ON)

# Plugin SDK (header-only)
add_subdirectory(plugin-sdk)

add_executable(PanoramaUi
    apps/journal/ui/stores/JournalStore.cpp
    apps/journal/ui/views/Journal.cpp
    apps/journal/ui/widgets/MarkdownEdit.cpp
    ui/MainWindow.cpp
    ui/RecentNodeStore.cpp
    ui/Recents.cpp
    ui/Toolbar.cpp
    ui/WidgetRegistry.cpp
    ui/main.cpp
    ui/resources.qrc
    ui/widgets/FileView.cpp

    # Required for MOC generation (HostContext has Q_OBJECT)
    plugin-sdk/include/HostContext.h
)
add_dependencies(PanoramaUi PanoramaCore)

add_library(PanoramaCoreLib STATIC IMPORTED GLOBAL)
set_target_properties(PanoramaCoreLib PROPERTIES
    IMPORTED_LOCATION "${RUST_LIB_FILE}"
)
add_dependencies(PanoramaCoreLib PanoramaCore)
target_link_libraries(PanoramaUi PRIVATE PanoramaCoreLib)

target_include_directories(PanoramaUi PRIVATE
    "${ads_SOURCE_DIR}/src"
    "${qmarkdowntextedit_SOURCE_DIR}"
    "${RUST_TARGET_DIR}/cxxbridge"
    "${CMAKE_SOURCE_DIR}/plugin-sdk/include"
)
target_link_libraries(PanoramaUi PRIVATE
    Qt6::Core
    Qt6::Widgets
    Qt6::Network
    Qt6::Pdf
    Qt6::PdfWidgets
    Qt6::WebSockets
    qtadvanceddocking-qt6
    qmarkdowntextedit
    plugin-sdk
)

if (CMAKE_SYSTEM_NAME STREQUAL "Windows")
    target_link_libraries(PanoramaUi PRIVATE
        bcrypt
        advapi32
        ntdll
    )
endif()

# Optionally build UI plugins found under apps/*/ui_plugin. This allows
# local app plugins (like apps/journals/ui_plugin) to be compiled with the
# main build. We only add the subdirectory when the plugin CMakeLists exists.
if(EXISTS "${CMAKE_SOURCE_DIR}/apps/journal/ui_plugin/CMakeLists.txt")
    add_subdirectory(apps/journal)
    add_dependencies(PanoramaUi journalsplugin)
endif()
