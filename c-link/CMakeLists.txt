cmake_minimum_required(VERSION 3.20)
project(c_link C)

find_package(OrbitC 0.1.0 CONFIG REQUIRED)
find_package(Threads REQUIRED)

add_executable(c_link main.c)
set_target_properties(c_link PROPERTIES OUTPUT_NAME c-link C_STANDARD 11 C_STANDARD_REQUIRED ON C_EXTENSIONS OFF)
target_compile_options(c_link PRIVATE -Wall -Wextra)
# The library's directory goes into the build RPATH, as -Wl,-rpath did.
target_link_libraries(c_link PRIVATE OrbitC::orbit Threads::Threads)
