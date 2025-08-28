cmake_minimum_required(VERSION 3.5)
project(journals_plugin)

find_package(Qt6 COMPONENTS Core Widgets REQUIRED)

add_library(journalsplugin SHARED
  JournalsPlugin.cpp
)

target_link_libraries(journalsplugin PRIVATE Qt6::Core Qt6::Widgets)

set_target_properties(journalsplugin PROPERTIES
  LIBRARY_OUTPUT_DIRECTORY ${CMAKE_CURRENT_SOURCE_DIR}
  PREFIX "lib"
)

