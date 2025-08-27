cmake_minimum_required(VERSION 3.31)
project(PanoramaCore)

find_program(CARGO_EXECUTABLE cargo)
if(NOT CARGO_EXECUTABLE)
    message(FATAL_ERROR "cargo not found")
endif()

if(CMAKE_BUILD_TYPE STREQUAL "Debug")
    set(RUST_BUILD_TYPE "debug")
    set(CARGO_BUILD_TYPE_FLAG "")
else()
    set(RUST_BUILD_TYPE "release")
    set(CARGO_BUILD_TYPE_FLAG "--release")
endif()

set(RUST_TARGET_DIR "${CMAKE_CURRENT_BINARY_DIR}/target/x86_64-pc-windows-gnu/${RUST_BUILD_TYPE}")
set(RUST_LIB_NAME panorama_core)
set(RUST_BRIDGE_FILE "${CMAKE_CURRENT_BINARY_DIR}/target/x86_64-pc-windows-gnu/cxxbridge/panorama-core/src/lib.rs.cc")

set(RUST_TARGET_FLAG "")
set(RUST_LIB_FILE "${RUST_TARGET_DIR}/libpanorama_core.a")
set(RUST_LIB_FILE "${RUST_LIB_FILE}" PARENT_SCOPE)
if (CMAKE_SYSTEM_NAME STREQUAL "Windows")
    set(RUST_TARGET_FLAG "--target" "x86_64-pc-windows-gnu")
    #    set(RUST_LIB_FILE "${RUST_TARGET_DIR}/panorama_core.lib")
else()
endif()

file(GLOB_RECURSE RUST_SOURCES
    "${CMAKE_CURRENT_SOURCE_DIR}/src/*.rs"
)

message(STATUS "WIN32 = ${WIN32}")
message(STATUS "CMAKE_SYSTEM_NAME = ${CMAKE_SYSTEM_NAME}")

message(STATUS "CARGO_BUILD_TYPE_FLAG = ${RUST_BUILD_TYPE_FLAG}")
message(STATUS "CARGO_TARGET_FLAG = ${RUST_TARGET_FLAG}")
message(STATUS "RUST_LIB_FILE = ${RUST_LIB_FILE}")
message(STATUS "RUST_BRIDGE_FILE = ${RUST_BRIDGE_FILE}")

add_custom_command(
    OUTPUT "${RUST_LIB_FILE}" "${RUST_BRIDGE_FILE}"
    COMMAND ${CARGO_EXECUTABLE} build --verbose ${RUST_BUILD_TYPE_FLAG} ${RUST_TARGET_FLAG} --target-dir "${CMAKE_CURRENT_BINARY_DIR}/target"
    WORKING_DIRECTORY ${CMAKE_CURRENT_SOURCE_DIR}
    DEPENDS ${RUST_SOURCES}
    COMMENT "Building Rust library"
)

add_custom_target(PanoramaCore ALL
    DEPENDS "${RUST_LIB_FILE}" "${RUST_BRIDGE_FILE}"
#    "${RUST_TARGET_DIR}/lib${RUST_LIB_NAME}.a"
)
