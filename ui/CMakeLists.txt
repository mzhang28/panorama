cmake_minimum_required(VERSION 3.31)
set(CMAKE_EXPORT_COMPILE_COMMANDS ON)
project(PanoramaUi LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

if(APPLE)
    message(STATUS "macOS detected, disabling Vulkan")
    set(Qt6_QT_FEATURE_vulkan OFF CACHE BOOL "" FORCE)
    set(CMAKE_DISABLE_FIND_PACKAGE_Vulkan ON)
endif()

find_package(Qt6 REQUIRED COMPONENTS Widgets)

include(FetchContent)
FetchContent_Declare(
    ADS
    GIT_REPOSITORY https://github.com/githubuser0xFFFF/Qt-Advanced-Docking-System.git
    GIT_TAG        4.4.0
)
FetchContent_MakeAvailable(ADS)

set(BUILD_EXAMPLES OFF CACHE BOOL "Do not build examples" FORCE)

set(CMAKE_AUTOMOC ON)
set(CMAKE_AUTORCC ON)
set(CMAKE_AUTOUIC ON)

add_executable(PanoramaUi
    src/main.cpp
    src/MainWindow.cpp
)

target_include_directories(PanoramaUi PRIVATE ${ads_SOURCE_DIR}/src)
target_link_libraries(PanoramaUi PRIVATE
    Qt6::Widgets
    qtadvanceddocking-qt6
)
