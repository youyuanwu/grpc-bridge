cmake_minimum_required(VERSION 3.20)
# configure vcpkg
# we use CmakePresets.json to point to vcpkg
if ("$ENV{VCPKG_ROOT}" STREQUAL "")
message(FATAL_ERROR "VCPKG_ROOT not found")
endif()

project(grpc-bridge VERSION 0.0.1 LANGUAGES CXX)

message(STATUS "vcpkg installed dir: ${VCPKG_INSTALLED_DIR}")

find_package(Protobuf REQUIRED)
find_package(gRPC CONFIG REQUIRED)

# Cmake find modules
#list(APPEND CMAKE_MODULE_PATH "${VCPKG_INSTALLED_DIR}/share/grpc")

add_subdirectory(examples)