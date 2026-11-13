cmake_minimum_required(VERSION 3.20)
project(uv_link LANGUAGES C)

find_package(OrbitC 0.1.0 CONFIG REQUIRED)
find_package(PkgConfig REQUIRED)
pkg_check_modules(LIBUV REQUIRED IMPORTED_TARGET libuv)

add_executable(uv_link main.c)
set_target_properties(uv_link PROPERTIES OUTPUT_NAME uv-link C_STANDARD 11 C_STANDARD_REQUIRED ON C_EXTENSIONS OFF)
target_compile_options(uv_link PRIVATE -Wall -Wextra -Werror)
target_link_libraries(uv_link PRIVATE OrbitC::orbit PkgConfig::LIBUV)
