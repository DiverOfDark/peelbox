cmake_minimum_required(VERSION 3.15)
project(app VERSION 1.0.0)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

add_executable(app
    src/main.cpp
)

# Statically link libstdc++ and libgcc so the runtime image only needs glibc
target_link_libraries(app
    -static-libstdc++
    -static-libgcc
    pthread
)
