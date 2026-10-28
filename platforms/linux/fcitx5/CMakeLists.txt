cmake_minimum_required(VERSION 3.22)
project(bopomofo_fcitx5 VERSION 0.1.0 LANGUAGES CXX)

if(NOT CMAKE_SYSTEM_NAME STREQUAL "Linux" OR CMAKE_CROSSCOMPILING)
    message(FATAL_ERROR "Build and run ABI checks on a native Linux target")
endif()
if(CMAKE_INSTALL_PREFIX_INITIALIZED_TO_DEFAULT)
    set(CMAKE_INSTALL_PREFIX "${CMAKE_BINARY_DIR}/bopomofo-fcitx5-dev" CACHE PATH "Isolated addon prefix" FORCE)
endif()
# Deliberately do not use FCITX_INSTALL_*: those can point to system directories.
find_package(Fcitx5Core 5.1.7 REQUIRED)
find_package(nlohmann_json 3.11 REQUIRED)
find_package(PkgConfig REQUIRED)
pkg_check_modules(XKBCOMMON REQUIRED IMPORTED_TARGET xkbcommon>=1.0)
find_package(Threads REQUIRED)
find_program(CARGO_EXECUTABLE cargo REQUIRED)
include(CTest)

get_filename_component(REPOSITORY_ROOT "${CMAKE_CURRENT_SOURCE_DIR}/../../.." ABSOLUTE)
set(BOPOMOFO_LEXICON "${REPOSITORY_ROOT}/fixtures/lexicon.tsv" CACHE FILEPATH "Approved normalized lexicon input")
if(NOT EXISTS "${BOPOMOFO_LEXICON}")
    message(FATAL_ERROR "BOPOMOFO_LEXICON must name an approved normalized lexicon")
endif()
set(BOPOMOFO_LEXICON_FILE "${CMAKE_INSTALL_PREFIX}/share/fcitx5/bopomofo-dev/lexicon.tsv")
configure_file(src/config.h.in config.h @ONLY)
configure_file(config/bopomofo-dev-addon.conf.in bopomofo-dev-addon.conf @ONLY)
configure_file(config/bopomofo-dev-inputmethod.conf bopomofo-dev-inputmethod.conf COPYONLY)
configure_file("${BOPOMOFO_LEXICON}" lexicon.tsv COPYONLY)
file(SHA256 "${CMAKE_BINARY_DIR}/lexicon.tsv" LEXICON_SHA256)
file(WRITE "${CMAKE_BINARY_DIR}/lexicon.sha256" "${LEXICON_SHA256}\n")
configure_file(scripts/prefix.txt.in prefix.txt @ONLY)

# Always build header and archive from this checkout. --locked prevents an
# implicit dependency update. The ABI executable is also a build-time gate.
file(GLOB_RECURSE RUST_INPUTS CONFIGURE_DEPENDS "${REPOSITORY_ROOT}/crates/ime-core/src/*.rs" "${REPOSITORY_ROOT}/crates/ime-ffi/src/*.rs")
set(RUST_LIBRARY "${CMAKE_BINARY_DIR}/rust/release/libime_ffi.a")
add_custom_command(OUTPUT "${RUST_LIBRARY}"
    COMMAND "${CARGO_EXECUTABLE}" build --locked --release -p ime-ffi --target-dir "${CMAKE_BINARY_DIR}/rust"
    WORKING_DIRECTORY "${REPOSITORY_ROOT}"
    DEPENDS ${RUST_INPUTS} "${REPOSITORY_ROOT}/Cargo.toml" "${REPOSITORY_ROOT}/Cargo.lock"
        "${REPOSITORY_ROOT}/rust-toolchain.toml" "${REPOSITORY_ROOT}/crates/ime-core/Cargo.toml"
        "${REPOSITORY_ROOT}/crates/ime-ffi/Cargo.toml"
    VERBATIM)
add_custom_target(bopomofo_rust DEPENDS "${RUST_LIBRARY}")
add_library(bopomofo_ffi STATIC IMPORTED)
set_target_properties(bopomofo_ffi PROPERTIES IMPORTED_LOCATION "${RUST_LIBRARY}"
    INTERFACE_INCLUDE_DIRECTORIES "${REPOSITORY_ROOT}/crates/ime-ffi/include")
add_dependencies(bopomofo_ffi bopomofo_rust)

add_library(bopomofo_adapter STATIC src/core_client.cpp src/keymap.cpp src/addon.cpp)
set_target_properties(bopomofo_adapter PROPERTIES POSITION_INDEPENDENT_CODE ON)
target_compile_features(bopomofo_adapter PUBLIC cxx_std_17)
target_compile_options(bopomofo_adapter PRIVATE -Wall -Wextra -Wpedantic -Werror)
target_include_directories(bopomofo_adapter PUBLIC src)
target_link_libraries(bopomofo_adapter PUBLIC bopomofo_ffi Fcitx5::Core nlohmann_json::nlohmann_json
    PkgConfig::XKBCOMMON Threads::Threads ${CMAKE_DL_LIBS} m)

add_executable(bopomofo_abi_check tests/abi_version.cpp)
target_link_libraries(bopomofo_abi_check PRIVATE bopomofo_ffi Threads::Threads ${CMAKE_DL_LIBS} m)
add_custom_target(bopomofo_abi_gate
    COMMAND $<TARGET_FILE:bopomofo_abi_check>
    DEPENDS bopomofo_abi_check VERBATIM)
add_library(bopomofo-dev MODULE src/factory.cpp)
target_include_directories(bopomofo-dev PRIVATE "${CMAKE_BINARY_DIR}")
target_link_libraries(bopomofo-dev PRIVATE bopomofo_adapter)
target_compile_options(bopomofo-dev PRIVATE -Wall -Wextra -Wpedantic -Werror)
target_link_options(bopomofo-dev PRIVATE -Wl,--no-undefined -Wl,--exclude-libs,ALL)
add_dependencies(bopomofo-dev bopomofo_abi_gate)

if(BUILD_TESTING)
    add_test(NAME abi_version COMMAND bopomofo_abi_check)
    add_executable(bopomofo_contract tests/contract.cpp)
    target_link_libraries(bopomofo_contract PRIVATE bopomofo_adapter)
    target_compile_options(bopomofo_contract PRIVATE -Wall -Wextra -Wpedantic -Werror)
    file(GLOB REPLAY_FIXTURES CONFIGURE_DEPENDS "${REPOSITORY_ROOT}/fixtures/*.jsonl")
    add_test(NAME adapter_contract COMMAND bopomofo_contract "${CMAKE_BINARY_DIR}/lexicon.tsv"
        "${CMAKE_BINARY_DIR}/lexicon.sha256" ${REPLAY_FIXTURES})
    add_test(NAME install_dry_run COMMAND bash "${CMAKE_CURRENT_SOURCE_DIR}/scripts/install.sh" "${CMAKE_BINARY_DIR}")
    add_test(NAME uninstall_dry_run COMMAND bash "${CMAKE_CURRENT_SOURCE_DIR}/scripts/uninstall.sh" "${CMAKE_BINARY_DIR}")
endif()

install(TARGETS bopomofo-dev LIBRARY DESTINATION lib/fcitx5)
install(FILES "${CMAKE_BINARY_DIR}/bopomofo-dev-addon.conf" DESTINATION share/fcitx5/addon RENAME bopomofo-dev.conf)
install(FILES "${CMAKE_BINARY_DIR}/bopomofo-dev-inputmethod.conf" DESTINATION share/fcitx5/inputmethod RENAME bopomofo-dev.conf)
install(FILES "${CMAKE_BINARY_DIR}/lexicon.tsv" "${CMAKE_BINARY_DIR}/lexicon.sha256" DESTINATION share/fcitx5/bopomofo-dev)
