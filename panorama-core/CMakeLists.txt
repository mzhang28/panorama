cmake_minimum_required(VERSION 3.31)
project(PanoramaCore)

find_program(CARGO_EXECUTABLE cargo)
if(NOT CARGO_EXECUTABLE)
    message(FATAL_ERROR "cargo not found")
endif()

if(CMAKE_BUILD_TYPE STREQUAL "Debug")
    set(RUST_BUILD_TYPE "debug")
    set(RUST_BUILD_TYPE_FLAG "")
else()
    set(RUST_BUILD_TYPE "release")
    set(RUST_BUILD_TYPE_FLAG "--release")
endif()

set(RUST_TARGET_FLAG "")
set(RUST_TARGET_SUFFIX "")
if (CMAKE_SYSTEM_NAME STREQUAL "Windows")
    # set(RUST_TARGET_FLAG "--target" "x86_64-pc-windows-msvc")
    # set(RUST_TARGET_SUFFIX "/x86_64-pc-windows-msvc")
else()
endif()

set(RUST_TARGET_DIR "${CMAKE_CURRENT_BINARY_DIR}/target${RUST_TARGET_SUFFIX}")
set(RUST_TARGET_DIR "${RUST_TARGET_DIR}" PARENT_SCOPE)
set(RUST_LIB_NAME panorama_core)
set(RUST_BRIDGE_FILE "${RUST_TARGET_DIR}/${RUST_BUILD_TYPE}/cxxbridge/panorama-core/src/lib.rs.cc")

if(MSVC)
    set(RUST_LIB_FILE "${RUST_TARGET_DIR}/${RUST_BUILD_TYPE}/panorama_core.lib")
else()
    set(RUST_LIB_FILE "${RUST_TARGET_DIR}/${RUST_BUILD_TYPE}/libpanorama_core.a")
endif()
set(RUST_LIB_FILE "${RUST_LIB_FILE}" PARENT_SCOPE)

file(GLOB_RECURSE RUST_SOURCES
    "${CMAKE_CURRENT_SOURCE_DIR}/src/*.rs"
)

message(STATUS "RUST_TARGET_DIR: ${RUST_TARGET_DIR}")
message(STATUS "RUST_TARGET_FLAG: ${RUST_TARGET_FLAG}")
message(STATUS "RUST_BUILD_TYPE_FLAG: ${RUST_BUILD_TYPE_FLAG}")

add_custom_command(
    OUTPUT "${RUST_LIB_FILE}" "${RUST_BRIDGE_FILE}"
    COMMAND ${CARGO_EXECUTABLE} build ${RUST_TARGET_FLAG} ${RUST_BUILD_TYPE_FLAG} --target-dir "${CMAKE_CURRENT_BINARY_DIR}/target"
    WORKING_DIRECTORY ${CMAKE_CURRENT_SOURCE_DIR}
    DEPENDS ${RUST_SOURCES}
    COMMENT "Building Rust library"
)

add_custom_target(PanoramaCore ALL
    DEPENDS "${RUST_LIB_FILE}" "${RUST_BRIDGE_FILE}"
)
