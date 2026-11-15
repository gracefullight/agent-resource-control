cmake_minimum_required(VERSION 3.20)
project(arc_native_fixture LANGUAGES C)
enable_testing()
add_executable(fixture fixture.c)
add_custom_target(native_barrier ALL
  COMMAND $<TARGET_FILE:fixture> build [=[literal $HOME ' 日本語]=]
  WORKING_DIRECTORY "${CMAKE_CURRENT_SOURCE_DIR}"
  DEPENDS fixture VERBATIM)
add_test(NAME native_case COMMAND fixture test [=[literal $HOME ' 日本語]=])
set_tests_properties(native_case PROPERTIES
  LABELS "unit;offline" TIMEOUT 120
  WORKING_DIRECTORY "${CMAKE_CURRENT_SOURCE_DIR}")
