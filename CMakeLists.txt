cmake_minimum_required(VERSION 3.31)
set(CMAKE_EXPORT_COMPILE_COMMANDS ON)
project(PanoramaUi LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(Qt6Core_USE_LIBINTL OFF)

if(APPLE)
    message(STATUS "macOS detected, disabling Vulkan")
    set(Qt6_QT_FEATURE_vulkan OFF CACHE BOOL "" FORCE)
    set(CMAKE_DISABLE_FIND_PACKAGE_Vulkan ON)
endif()

find_package(Qt6 REQUIRED COMPONENTS Widgets Network)

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

add_subdirectory(panorama-core)
if(CMAKE_BUILD_TYPE STREQUAL "Debug")
    set(RUST_BUILD_TYPE "debug")
else()
    set(RUST_BUILD_TYPE "release")
endif()

set(CMAKE_AUTOMOC ON)
set(CMAKE_AUTORCC ON)
set(CMAKE_AUTOUIC ON)

add_executable(PanoramaUi
    ui/MainWindow.cpp
    ui/Recents.cpp
    ui/RecentNodeStore.cpp
    ui/Toolbar.cpp
    ui/main.cpp
    ui/resources.qrc
    ui/views/Journal.cpp
    ui/stores/JournalStore.cpp
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
)
target_link_libraries(PanoramaUi PRIVATE
    Qt6::Widgets
    Qt6::Network
    qtadvanceddocking-qt6
    qmarkdowntextedit
)

if (CMAKE_SYSTEM_NAME STREQUAL "Windows")
    target_link_libraries(PanoramaUi PRIVATE
        bcrypt
        advapi32
        ntdll
    )
endif()
