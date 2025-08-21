cmake_minimum_required(VERSION 3.31)
project(PanoramaCore)

# Path to Cargo
find_program(CARGO_EXECUTABLE cargo)

if(NOT CARGO_EXECUTABLE)
    message(FATAL_ERROR "cargo not found")
endif()

set(RUST_TARGET_DIR "${CMAKE_CURRENT_BINARY_DIR}/target/release")
set(RUST_LIB_NAME panorama_core) # must match Cargo.toml [lib] name

file(GLOB_RECURSE RUST_SOURCES
    "${CMAKE_CURRENT_SOURCE_DIR}/src/*.rs"
)

add_custom_command(
    OUTPUT "${RUST_TARGET_DIR}/lib${RUST_LIB_NAME}.a"
    COMMAND ${CARGO_EXECUTABLE} build --release --target-dir "${CMAKE_CURRENT_BINARY_DIR}/target"
    WORKING_DIRECTORY ${CMAKE_CURRENT_SOURCE_DIR}
    COMMENT "Building Rust library"
    DEPENDS ${RUST_SOURCES}
)

add_custom_target(PanoramaCore ALL
    DEPENDS "${RUST_TARGET_DIR}/lib${RUST_LIB_NAME}.a"
)
