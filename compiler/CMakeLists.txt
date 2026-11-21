# SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
#
# Unlo\n, Unio's compiler, its runtime, its unit tests and the test runner in ../tools (D131, Building rule 9).

cmake_minimum_required(VERSION 3.28)
project(unio LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 23)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_CXX_EXTENSIONS OFF)
set(CMAKE_EXPORT_COMPILE_COMMANDS ON)

# One pinned LLVM (Building rule 9): the self-contained static build of 23.1.2, which nothing upgrades under us.
set(UNIO_LLVM_PREFIX "$ENV{HOME}/llvm-self-contained/install" CACHE PATH "the pinned LLVM 23.1.2")
find_package(LLVM 23.1.2 EXACT REQUIRED CONFIG PATHS "${UNIO_LLVM_PREFIX}/lib/cmake/llvm" NO_DEFAULT_PATH)

option(UNIO_SANITIZE "AddressSanitizer and UndefinedBehaviorSanitizer (Building rule 3)" ON)
option(UNIO_TIDY "run clang-tidy while compiling (Building rule 3)" OFF)

# Building rule 3: every warning, as errors; libc++ hardened in every build; no exceptions, matching LLVM, and no
# RTTI, since the pinned LLVM is built without it.
set(UNIO_WARNINGS -Wall -Wextra -Wpedantic -Wconversion -Wshadow -Werror)
set(UNIO_COMMON_FLAGS -fno-exceptions -fno-rtti -D_LIBCPP_HARDENING_MODE=_LIBCPP_HARDENING_MODE_EXTENSIVE)
if(UNIO_SANITIZE)
  set(UNIO_SANITIZER_FLAGS -fsanitize=address,undefined -fno-sanitize-recover=all -fno-omit-frame-pointer)
endif()

if(UNIO_TIDY)
  find_program(UNIO_CLANG_TIDY clang-tidy PATHS /opt/homebrew/opt/llvm@23/bin NO_DEFAULT_PATH REQUIRED)
  set(UNIO_TIDY_COMMAND "${UNIO_CLANG_TIDY}" "--config-file=${CMAKE_CURRENT_SOURCE_DIR}/.clang-tidy")
endif()

function(unio_target_options target)
  target_compile_options(${target} PRIVATE ${UNIO_WARNINGS} ${UNIO_COMMON_FLAGS} ${UNIO_SANITIZER_FLAGS})
  target_link_options(${target} PRIVATE ${UNIO_SANITIZER_FLAGS})
  if(UNIO_TIDY)
    set_target_properties(${target} PROPERTIES CXX_CLANG_TIDY "${UNIO_TIDY_COMMAND}")
  endif()
endfunction()

# The runtime every Unio program is linked with. In a sanitized build it's sanitized too, and programs are linked with
# the sanitizers' runtime, so every test's program runs under them as well (Building rule 3).
add_library(unio_runtime STATIC runtime/runtime.cpp)
target_compile_options(unio_runtime PRIVATE ${UNIO_WARNINGS} -fno-exceptions -fno-rtti -O2 -mmacosx-version-min=14.0
  -D_LIBCPP_HARDENING_MODE=_LIBCPP_HARDENING_MODE_FAST ${UNIO_SANITIZER_FLAGS})
set_target_properties(unio_runtime PROPERTIES OUTPUT_NAME unio_runtime ARCHIVE_OUTPUT_DIRECTORY "${CMAKE_BINARY_DIR}")
if(UNIO_TIDY)
  set_target_properties(unio_runtime PROPERTIES CXX_CLANG_TIDY "${UNIO_TIDY_COMMAND}")
endif()
set(UNIO_SANITIZER_LIBRARY_DIR "")
if(UNIO_SANITIZE)
  execute_process(COMMAND "${CMAKE_CXX_COMPILER}" -print-resource-dir OUTPUT_VARIABLE UNIO_RESOURCE_DIR
    OUTPUT_STRIP_TRAILING_WHITESPACE)
  set(UNIO_SANITIZER_LIBRARY_DIR "${UNIO_RESOURCE_DIR}/lib/darwin")
endif()

llvm_map_components_to_libnames(UNIO_LLVM_LIBS core support passes target targetparser mc irreader bitwriter
  aarch64codegen aarch64asmparser aarch64desc aarch64info x86codegen x86asmparser x86desc x86info)

# The compiler as a library, so the unit tests reach Unio IR and its passes directly.
add_library(unlo STATIC
  source/source.cpp
  source/messages.cpp
  source/lexer.cpp
  source/parser.cpp
  source/checker.cpp
  source/ir.cpp
  source/lower.cpp
  source/optimize.cpp
  source/codegen.cpp
  source/backend.cpp
  source/driver.cpp)
target_include_directories(unlo PUBLIC source)
target_include_directories(unlo SYSTEM PUBLIC ${LLVM_INCLUDE_DIRS})
target_compile_definitions(unlo PUBLIC ${LLVM_DEFINITIONS_LIST}
  UNIO_SDK_PATH="${CMAKE_OSX_SYSROOT}" UNIO_SANITIZER_LIBRARY_DIR="${UNIO_SANITIZER_LIBRARY_DIR}")
target_link_libraries(unlo PUBLIC ${UNIO_LLVM_LIBS})
unio_target_options(unlo)

add_executable(unio source/main.cpp)
target_link_libraries(unio PRIVATE unlo)
add_dependencies(unio unio_runtime)
unio_target_options(unio)

add_executable(unit_tests unit_tests/unit_tests.cpp)
target_link_libraries(unit_tests PRIVATE unlo)
unio_target_options(unit_tests)

add_executable(test_runner ../tools/test_runner.cpp)
unio_target_options(test_runner)

# Formatting is checked, never changed by a check run (Building rule 3).
find_program(UNIO_CLANG_FORMAT clang-format PATHS /opt/homebrew/opt/llvm@23/bin NO_DEFAULT_PATH REQUIRED)
file(GLOB UNIO_FORMATTED CONFIGURE_DEPENDS source/*.cpp source/*.h runtime/*.cpp runtime/*.h unit_tests/*.cpp
  ../tools/*.cpp)

enable_testing()
add_test(NAME format COMMAND "${UNIO_CLANG_FORMAT}" --dry-run --Werror ${UNIO_FORMATTED}
  WORKING_DIRECTORY "${CMAKE_CURRENT_SOURCE_DIR}")
add_test(NAME unit_tests COMMAND unit_tests)
add_test(NAME unio_tests COMMAND test_runner
  --unio $<TARGET_FILE:unio>
  --tests "${CMAKE_CURRENT_SOURCE_DIR}/../tests"
  --status "${CMAKE_CURRENT_SOURCE_DIR}/test-status.txt"
  --jobs 3)
