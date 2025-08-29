cmake_minimum_required(VERSION 3.5)
project(plugin-sdk)

# Header-only target exporting include path for the plugin SDK
add_library(plugin-sdk INTERFACE)
target_include_directories(plugin-sdk INTERFACE
  $<INSTALL_INTERFACE:include>
  $<BUILD_INTERFACE:${CMAKE_CURRENT_SOURCE_DIR}/include>
)
