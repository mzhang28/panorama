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

set(RUST_TARGET_DIR "${CMAKE_CURRENT_BINARY_DIR}/target/${RUST_BUILD_TYPE}")
set(RUST_LIB_NAME panorama_core)

file(GLOB_RECURSE RUST_SOURCES
    "${CMAKE_CURRENT_SOURCE_DIR}/src/*.rs"
)

add_custom_command(
    OUTPUT "${RUST_TARGET_DIR}/lib${RUST_LIB_NAME}.a"
    COMMAND ${CARGO_EXECUTABLE} build ${CARGO_BUILD_TYPE_FLAG} --target-dir "${CMAKE_CURRENT_BINARY_DIR}/target"
    WORKING_DIRECTORY ${CMAKE_CURRENT_SOURCE_DIR}
    DEPENDS ${RUST_SOURCES}
    COMMENT "Building Rust library"
)

add_custom_target(PanoramaCore ALL
    DEPENDS "${RUST_TARGET_DIR}/lib${RUST_LIB_NAME}.a"
)
