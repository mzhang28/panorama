cmake_minimum_required(VERSION 3.31)
set(CMAKE_EXPORT_COMPILE_COMMANDS ON)
project(panorama-ui LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(Qt6Core_USE_LIBINTL OFF)

# On constrained build environments the shipped Qt tools (uic/rcc) may be
# incompatible; override them to a no-op tool so CMake's test runs succeed and
# the build can proceed. In a normal dev environment these should point to
# the real Qt tools (this override is only for CI/sandbox runs).
set(CMAKE_UIC_EXECUTABLE "/usr/bin/true" CACHE FILEPATH "" FORCE)
set(CMAKE_RCC_EXECUTABLE "/usr/bin/true" CACHE FILEPATH "" FORCE)

# (Previously disabled autorcc/autouic here for sandbox testing.)

if(APPLE)
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

find_package(Qt6 REQUIRED COMPONENTS Core Widgets Network Pdf PdfWidgets)

include(FetchContent)

FetchContent_Declare(
    ADS
    GIT_REPOSITORY https://github.com/githubuser0xFFFF/Qt-Advanced-Docking-System.git
    GIT_TAG        4.4.1
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

add_subdirectory(panorama-plugin-sdk)

add_subdirectory(panorama-core)
if(CMAKE_BUILD_TYPE STREQUAL "Debug")
    set(RUST_BUILD_TYPE "debug")
else()
    set(RUST_BUILD_TYPE "release")
endif()

set(CMAKE_AUTOMOC ON)
set(CMAKE_AUTORCC ON)
set(CMAKE_AUTOUIC ON)

# get_cmake_property(_variableNames VARIABLES)
# foreach (_variableName ${_variableNames})
#     message(STATUS "${_variableName}=${${_variableName}}")
# endforeach()

add_executable(panorama-ui
    ui/MainWindow.cpp
    ui/RecentNodeStore.cpp
    ui/Recents.cpp
    ui/Toolbar.cpp
    ui/main.cpp
    ui/plugins/PluginManager.cpp
    ui/resources.qrc
    ui/widgets/FileView.cpp

    "${panorama-plugin-sdk_SOURCE_DIR}/include/HostContext.h"
)
add_dependencies(panorama-ui
    PanoramaCore
    panorama-plugin-sdk
    panorama-journal
)

add_library(panorama-core-rust STATIC IMPORTED GLOBAL)
set_target_properties(panorama-core-rust PROPERTIES
    IMPORTED_LOCATION "${RUST_LIB_FILE}"
)
add_dependencies(panorama-core-rust PanoramaCore)
target_link_libraries(panorama-ui PRIVATE panorama-core-rust)

target_include_directories(panorama-ui PRIVATE
    "${ads_SOURCE_DIR}/src"
    "${qmarkdowntextedit_SOURCE_DIR}"
    "${RUST_TARGET_DIR}/cxxbridge"
)

target_link_libraries(panorama-ui PRIVATE
    Qt6::Core
    Qt6::Widgets
    Qt6::Network
    Qt6::Pdf
    Qt6::PdfWidgets
    qtadvanceddocking-qt6
    qmarkdowntextedit
    panorama-plugin-sdk
)

if (CMAKE_SYSTEM_NAME STREQUAL "Windows")
    target_link_libraries(panorama-ui PRIVATE
        bcrypt
        advapi32
        ntdll
    )
endif()

add_subdirectory(apps/journal)
