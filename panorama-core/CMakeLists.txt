cmake_minimum_required(VERSION 3.31)
project(PanoramaCore)

find_program(CARGO_EXECUTABLE cargo)
if(NOT CARGO_EXECUTABLE)
    message(FATAL_ERROR "cargo not found")
endif()

# Rust library name and target directory
set(RUST_LIB_NAME panorama_core)
set(RUST_TARGET_DIR "${CMAKE_CURRENT_BINARY_DIR}/target")
set(RUST_TARGET_DIR "${RUST_TARGET_DIR}" PARENT_SCOPE)

# Rust library extension based on compiler
if(MSVC)
    set(RUST_LIB_EXT "lib")
else()
    set(RUST_LIB_EXT "a")
endif()

# Rust bridge file and library path using runtime build config
set(RUST_BRIDGE_FILE "${RUST_TARGET_DIR}/$<CONFIG>/cxxbridge/panorama-core/src/lib.rs.cc")
set(RUST_LIB_FILE "${RUST_TARGET_DIR}/$<CONFIG>/${RUST_LIB_NAME}.${RUST_LIB_EXT}")
set(RUST_LIB_FILE "${RUST_LIB_FILE}" PARENT_SCOPE)

# Gather all Rust source files
file(GLOB_RECURSE RUST_SOURCES
    "${CMAKE_CURRENT_SOURCE_DIR}/src/*.rs"
)

# Optional debug messages
message(STATUS "RUST_TARGET_DIR: ${RUST_TARGET_DIR}")
message(STATUS "RUST_LIB_FILE: ${RUST_LIB_FILE}")
message(STATUS "RUST_BRIDGE_FILE: ${RUST_BRIDGE_FILE}")

# Build Rust library
add_custom_command(
    OUTPUT "${RUST_LIB_FILE}" "${RUST_BRIDGE_FILE}"
    COMMAND ${CARGO_EXECUTABLE} build $<$<CONFIG:Release>:--release>
    WORKING_DIRECTORY ${CMAKE_CURRENT_SOURCE_DIR}
    DEPENDS ${RUST_SOURCES}
    COMMENT "Building Rust library for $<CONFIG>"
)

# Custom target depends on Rust library
add_custom_target(PanoramaCore ALL
    DEPENDS "${RUST_LIB_FILE}" "${RUST_BRIDGE_FILE}"
)
