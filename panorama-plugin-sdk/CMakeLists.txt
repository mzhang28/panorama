cmake_minimum_required(VERSION 3.31)
set(CMAKE_EXPORT_COMPILE_COMMANDS ON)
project(panorama-plugin-sdk LANGUAGES CXX)

#---------------------------------------------------------------------------------------------------------
# Dependencies

find_package(Qt6 REQUIRED COMPONENTS Core)

#---------------------------------------------------------------------------------------------------------
# Library

add_library(panorama-plugin-sdk INTERFACE)

target_include_directories(panorama-plugin-sdk INTERFACE
  $<INSTALL_INTERFACE:include>
  $<BUILD_INTERFACE:${CMAKE_CURRENT_SOURCE_DIR}/include>
)

target_link_libraries(panorama-plugin-sdk INTERFACE Qt6::Core)
