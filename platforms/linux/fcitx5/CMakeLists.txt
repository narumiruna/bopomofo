cmake_minimum_required(VERSION 3.22)
project(bopomofo_fcitx5 VERSION 0.1.0 LANGUAGES CXX)
if(NOT CMAKE_SYSTEM_NAME STREQUAL "Linux" OR CMAKE_CROSSCOMPILING)
    message(FATAL_ERROR "Build and run ABI checks on a native Linux target")
endif()
if(CMAKE_INSTALL_PREFIX_INITIALIZED_TO_DEFAULT)
    set(CMAKE_INSTALL_PREFIX "${CMAKE_BINARY_DIR}/bopomofo-fcitx5-dev" CACHE PATH "Isolated addon prefix" FORCE)
endif()
find_package(Fcitx5Core 5.1.7 REQUIRED)
find_package(nlohmann_json 3.11 REQUIRED)
find_package(PkgConfig REQUIRED)
pkg_check_modules(XKBCOMMON REQUIRED IMPORTED_TARGET xkbcommon>=1.0)
find_package(Threads REQUIRED)
find_program(CARGO_EXECUTABLE cargo REQUIRED)
include(CTest)
get_filename_component(REPOSITORY_ROOT "${CMAKE_CURRENT_SOURCE_DIR}/../../.." ABSOLUTE)
set(BOPOMOFO_ASSET_SOURCE "${REPOSITORY_ROOT}/data/libchewing/source" CACHE FILEPATH "Approved authoring TSV or pinned upstream source directory")
file(GLOB_RECURSE RUST_INPUTS CONFIGURE_DEPENDS "${REPOSITORY_ROOT}/crates/*/src/*.rs")
set_property(DIRECTORY APPEND PROPERTY CMAKE_CONFIGURE_DEPENDS ${RUST_INPUTS}
    "${REPOSITORY_ROOT}/Cargo.toml" "${REPOSITORY_ROOT}/Cargo.lock"
    "${REPOSITORY_ROOT}/crates/ime-cli/Cargo.toml" "${REPOSITORY_ROOT}/crates/ime-core/Cargo.toml"
    "${REPOSITORY_ROOT}/data/libchewing/source/word.csv" "${REPOSITORY_ROOT}/data/libchewing/source/tsi.csv"
    "${REPOSITORY_ROOT}/data/libchewing/COPYING" "${REPOSITORY_ROOT}/data/libchewing/AUTHORS"
    "${REPOSITORY_ROOT}/data/libchewing/NOTICE.md" "${REPOSITORY_ROOT}/LICENSE"
    "${REPOSITORY_ROOT}/fixtures/lexicon.tsv" "${REPOSITORY_ROOT}/data/cns11643/lexicon.tsv"
    "${REPOSITORY_ROOT}/fixtures/lexicon-quality/lexicon.tsv" "${REPOSITORY_ROOT}/fixtures/unicode/lexicon.tsv"
    "${REPOSITORY_ROOT}/data/cns11643/NOTICE.txt" "${REPOSITORY_ROOT}/data/cns11643/LICENSE.txt")
# Build data only in this build directory. No native installation occurs.
execute_process(COMMAND "${CARGO_EXECUTABLE}" build --locked --release -p ime-cli
    --target-dir "${CMAKE_BINARY_DIR}/rust" WORKING_DIRECTORY "${REPOSITORY_ROOT}"
    RESULT_VARIABLE CLI_RESULT)
if(NOT CLI_RESULT EQUAL 0)
    message(FATAL_ERROR "Asset builder failed")
endif()
function(build_assets source name)
    set(destination "${CMAKE_BINARY_DIR}/${name}")
    file(REMOVE_RECURSE "${destination}")
    execute_process(COMMAND "${CMAKE_BINARY_DIR}/rust/release/ime-cli" build-assets "${source}" "${destination}"
        RESULT_VARIABLE result)
    if(NOT result EQUAL 0)
        message(FATAL_ERROR "Approved asset profile failed: ${name}")
    endif()
endfunction()
build_assets("${BOPOMOFO_ASSET_SOURCE}" assets)
configure_file(src/config.h.in config.h @ONLY)
configure_file(config/bopomofo-dev-addon.conf.in bopomofo-dev-addon.conf @ONLY)
configure_file(config/bopomofo-dev-inputmethod.conf bopomofo-dev-inputmethod.conf COPYONLY)
configure_file(scripts/prefix.txt.in prefix.txt @ONLY)
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
add_custom_target(bopomofo_abi_gate COMMAND $<TARGET_FILE:bopomofo_abi_check>
    DEPENDS bopomofo_abi_check VERBATIM)
add_library(bopomofo-dev MODULE src/factory.cpp)
target_include_directories(bopomofo-dev PRIVATE "${CMAKE_BINARY_DIR}")
target_link_libraries(bopomofo-dev PRIVATE bopomofo_adapter)
target_compile_options(bopomofo-dev PRIVATE -Wall -Wextra -Wpedantic -Werror)
target_link_options(bopomofo-dev PRIVATE -Wl,--no-undefined -Wl,--exclude-libs,ALL)
add_dependencies(bopomofo-dev bopomofo_abi_gate)
if(BUILD_TESTING)
    build_assets("${REPOSITORY_ROOT}/fixtures/lexicon.tsv" fixture-assets)
    build_assets("${REPOSITORY_ROOT}/data/cns11643/lexicon.tsv" cns-assets)
    build_assets("${REPOSITORY_ROOT}/fixtures/lexicon-quality/lexicon.tsv" quality-assets)
    build_assets("${REPOSITORY_ROOT}/fixtures/unicode/lexicon.tsv" unicode-assets)
    add_test(NAME abi_version COMMAND bopomofo_abi_check)
    add_executable(bopomofo_abi_v1 tests/abi_v1.cpp)
    target_link_libraries(bopomofo_abi_v1 PRIVATE bopomofo_ffi Threads::Threads ${CMAKE_DL_LIBS} m)
    add_test(NAME reject_v1_host COMMAND bopomofo_abi_v1)
    set_tests_properties(reject_v1_host PROPERTIES WILL_FAIL TRUE)
    add_executable(bopomofo_contract tests/contract.cpp)
    target_link_libraries(bopomofo_contract PRIVATE bopomofo_adapter)
    target_compile_options(bopomofo_contract PRIVATE -Wall -Wextra -Wpedantic -Werror)
    file(GLOB REPLAY_FIXTURES CONFIGURE_DEPENDS "${REPOSITORY_ROOT}/fixtures/*.jsonl")
    list(REMOVE_ITEM REPLAY_FIXTURES "${REPOSITORY_ROOT}/fixtures/cns11643.jsonl")
    add_test(NAME adapter_contract COMMAND bopomofo_contract
        "${CMAKE_BINARY_DIR}/fixture-assets/dictionary.dat" "${CMAKE_BINARY_DIR}/fixture-assets/assets.json" ${REPLAY_FIXTURES})
    add_test(NAME known_gap_reproduced_dbus_stale_rendered_index COMMAND bopomofo_contract
        --known-gap-reproduced "${CMAKE_BINARY_DIR}/fixture-assets/dictionary.dat" "${CMAKE_BINARY_DIR}/fixture-assets/assets.json")
    set_tests_properties(known_gap_reproduced_dbus_stale_rendered_index PROPERTIES LABELS "known-gap;headless")
    add_test(NAME cns_replay COMMAND bopomofo_contract --replay-only
        "${CMAKE_BINARY_DIR}/cns-assets/dictionary.dat" "${CMAKE_BINARY_DIR}/cns-assets/assets.json" "${REPOSITORY_ROOT}/fixtures/cns11643.jsonl")
    add_test(NAME cns_replay_wrong_profile COMMAND bopomofo_contract --replay-only
        "${CMAKE_BINARY_DIR}/fixture-assets/dictionary.dat" "${CMAKE_BINARY_DIR}/fixture-assets/assets.json" "${REPOSITORY_ROOT}/fixtures/cns11643.jsonl")
    set_tests_properties(cns_replay_wrong_profile PROPERTIES WILL_FAIL TRUE)
    foreach(profile IN ITEMS quality unicode)
        if(profile STREQUAL "quality")
            set(cases "${REPOSITORY_ROOT}/fixtures/lexicon-quality/cases.jsonl")
        else()
            set(cases "${REPOSITORY_ROOT}/fixtures/unicode/cases.jsonl")
        endif()
        add_test(NAME ${profile}_replay COMMAND bopomofo_contract --replay-only
            "${CMAKE_BINARY_DIR}/${profile}-assets/dictionary.dat" "${CMAKE_BINARY_DIR}/${profile}-assets/assets.json" "${cases}")
    endforeach()
    add_test(NAME default_assets COMMAND "${CMAKE_BINARY_DIR}/rust/release/ime-cli" validate-assets "${CMAKE_BINARY_DIR}/assets")
    add_test(NAME install_dry_run COMMAND bash "${CMAKE_CURRENT_SOURCE_DIR}/scripts/install.sh" "${CMAKE_BINARY_DIR}")
    add_test(NAME uninstall_dry_run COMMAND bash "${CMAKE_CURRENT_SOURCE_DIR}/scripts/uninstall.sh" "${CMAKE_BINARY_DIR}")
endif()
install(TARGETS bopomofo-dev LIBRARY DESTINATION lib/fcitx5)
install(FILES "${CMAKE_BINARY_DIR}/bopomofo-dev-addon.conf" DESTINATION share/fcitx5/addon RENAME bopomofo-dev.conf)
install(FILES "${CMAKE_BINARY_DIR}/bopomofo-dev-inputmethod.conf" DESTINATION share/fcitx5/inputmethod RENAME bopomofo-dev.conf)
install(DIRECTORY "${CMAKE_BINARY_DIR}/assets/" DESTINATION share/fcitx5/bopomofo-dev)
