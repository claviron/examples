cmake_minimum_required(VERSION 3.20)
project(CModule LANGUAGES C CXX)

set(CMAKE_C_STANDARD 11)
set(CMAKE_CXX_STANDARD 17)

# Libraries loaded at run time, put where Pentaglot's modules.yaml names them.
add_library(hello MODULE hello.c)
add_library(hello_cpp MODULE hello_cpp.cpp)
foreach(library hello hello_cpp)
  target_include_directories(${library} PRIVATE include)
  target_compile_options(${library} PRIVATE -Wall -Wextra)
  set_target_properties(${library} PROPERTIES
    LIBRARY_OUTPUT_DIRECTORY "${CMAKE_CURRENT_SOURCE_DIR}/../pentaglot/modules")
endforeach()
