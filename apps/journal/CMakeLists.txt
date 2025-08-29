cmake_minimum_required(VERSION 3.5)
set(CMAKE_EXPORT_COMPILE_COMMANDS ON)
project(journals_plugin)

set(CMAKE_AUTOMOC ON)
set(CMAKE_AUTORCC ON)
set(CMAKE_AUTOUIC ON)

#---------------------------------------------------------------------------------------------------------
# Dependencies

find_package(Qt6 COMPONENTS Core Network Widgets WebSockets REQUIRED)

if(NOT TARGET PanoramaPluginSdk)
    if(EXISTS "${CMAKE_CURRENT_LIST_DIR}/../plugin-sdk/CMakeLists.txt")
        add_subdirectory("../plugin-sdk")
    endif()
endif()

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

add_library(journalsplugin SHARED
  ui/JournalsPlugin.cpp
  ui/stores/JournalStore.cpp
  ui/views/Journal.cpp
  ui/widgets/MarkdownEdit.cpp
  # "${CMAKE_SOURCE_DIR}/plugin-sdk/include/HostContext.h"
)

target_link_libraries(journalsplugin PRIVATE
    Qt6::Core
    Qt6::Widgets
    Qt6::Network
    Qt6::WebSockets
    qmarkdowntextedit
    plugin-sdk
)

target_include_directories(journalsplugin PRIVATE
    "${qmarkdowntextedit_SOURCE_DIR}"
    "${PanoramaPluginSdk_INCLUDE_DIR}"
)

set_target_properties(journalsplugin PROPERTIES
  LIBRARY_OUTPUT_DIRECTORY ${CMAKE_CURRENT_SOURCE_DIR}
  PREFIX "lib"
)
