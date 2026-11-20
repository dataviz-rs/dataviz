# dataviz for C and C++ with CMake.
#
# In your project:
#   add_subdirectory(path/to/dataviz/capi dataviz)
#   target_link_libraries(app PRIVATE dataviz::dataviz)
#
# Builds the Rust library with cargo (Rust 1.89+ needed). Options:
#   DATAVIZ_STATIC    link the static library instead of the shared one
#   DATAVIZ_FEATURES  cargo features, e.g. "gpu" or "" (default: window;pdf)
#   DATAVIZ_TESTS     build the C and C++ tests (ctest)
cmake_minimum_required(VERSION 3.16)
project(dataviz_c LANGUAGES C CXX)

option(DATAVIZ_STATIC "Link dataviz statically" OFF)
option(DATAVIZ_TESTS "Build the dataviz C/C++ tests" OFF)
set(DATAVIZ_FEATURES "window;pdf" CACHE STRING "Cargo features of dataviz-c")

find_program(CARGO cargo HINTS $ENV{HOME}/.cargo/bin $ENV{USERPROFILE}/.cargo/bin REQUIRED)

set(_root ${CMAKE_CURRENT_SOURCE_DIR}/..)
set(_target_dir ${CMAKE_CURRENT_BINARY_DIR}/cargo)
set(_out ${_target_dir}/release)
string(REPLACE ";" "," _features "${DATAVIZ_FEATURES}")

if(WIN32)
  if(MSVC)
    set(_shared ${_out}/dataviz_c.dll)
    set(_implib ${_out}/dataviz_c.dll.lib)
    set(_static ${_out}/dataviz_c.lib)
  else()
    set(_shared ${_out}/dataviz_c.dll)
    set(_implib ${_out}/libdataviz_c.dll.a)
    set(_static ${_out}/libdataviz_c.a)
  endif()
elseif(APPLE)
  set(_shared ${_out}/libdataviz_c.dylib)
  set(_static ${_out}/libdataviz_c.a)
else()
  set(_shared ${_out}/libdataviz_c.so)
  set(_static ${_out}/libdataviz_c.a)
endif()

add_custom_command(
  OUTPUT ${_shared} ${_static} ${_implib}
  COMMAND ${CARGO} build --profile release -p dataviz-c --no-default-features
          --features "${_features}" --target-dir ${_target_dir}
  WORKING_DIRECTORY ${_root}
  DEPENDS ${_root}/capi/src/lib.rs ${_root}/capi/Cargo.toml
  COMMENT "Building dataviz (cargo)"
  USES_TERMINAL)
add_custom_target(dataviz_cargo DEPENDS ${_shared} ${_static})

if(DATAVIZ_STATIC)
  add_library(dataviz_c STATIC IMPORTED GLOBAL)
  set_target_properties(dataviz_c PROPERTIES IMPORTED_LOCATION ${_static})
  if(WIN32)
    set(_sys ws2_32 userenv bcrypt ntdll advapi32 kernel32 user32 gdi32 ole32 shell32 winmm
             oleaut32 propsys runtimeobject d3dcompiler opengl32 dwmapi)
  elseif(APPLE)
    set(_sys "-framework Cocoa" "-framework Foundation" "-framework CoreFoundation"
             "-framework AppKit" "-framework CoreGraphics" "-framework CoreVideo"
             "-framework QuartzCore" "-framework Metal" "-framework IOKit" "-framework Security"
             objc iconv m)
  else()
    set(_sys dl pthread m rt util)
  endif()
  set_target_properties(dataviz_c PROPERTIES INTERFACE_LINK_LIBRARIES "${_sys}")
else()
  add_library(dataviz_c SHARED IMPORTED GLOBAL)
  set_target_properties(dataviz_c PROPERTIES IMPORTED_LOCATION ${_shared})
  if(WIN32)
    set_target_properties(dataviz_c PROPERTIES IMPORTED_IMPLIB ${_implib}
                          INTERFACE_COMPILE_DEFINITIONS DATAVIZ_SHARED)
  endif()
endif()
set_target_properties(dataviz_c PROPERTIES INTERFACE_INCLUDE_DIRECTORIES
                      ${CMAKE_CURRENT_SOURCE_DIR}/include)
add_dependencies(dataviz_c dataviz_cargo)
add_library(dataviz::dataviz ALIAS dataviz_c)

# The library file, e.g. to copy next to an executable on Windows.
set(DATAVIZ_SHARED_LIBRARY ${_shared} CACHE INTERNAL "")

if(DATAVIZ_TESTS)
  enable_testing()
  foreach(t c cpp)
    add_executable(dataviz_test_${t} tests/test.${t})
    target_link_libraries(dataviz_test_${t} PRIVATE dataviz::dataviz)
    if(NOT WIN32 AND t STREQUAL "c")
      target_link_libraries(dataviz_test_${t} PRIVATE m)
    endif()
    if(WIN32 AND NOT DATAVIZ_STATIC)
      add_custom_command(TARGET dataviz_test_${t} POST_BUILD
        COMMAND ${CMAKE_COMMAND} -E copy_if_different ${_shared} $<TARGET_FILE_DIR:dataviz_test_${t}>)
    endif()
    add_test(NAME dataviz_${t} COMMAND dataviz_test_${t} ${CMAKE_CURRENT_BINARY_DIR})
  endforeach()
  set_target_properties(dataviz_test_c PROPERTIES C_STANDARD 99)
  set_target_properties(dataviz_test_cpp PROPERTIES CXX_STANDARD 17)
  add_executable(dataviz_example_hello examples/hello.cpp)
  target_link_libraries(dataviz_example_hello PRIVATE dataviz::dataviz)
  set_target_properties(dataviz_example_hello PROPERTIES CXX_STANDARD 17)
endif()
