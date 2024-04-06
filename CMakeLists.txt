cmake_minimum_required(VERSION 3.20)
# configure vcpkg
# we use CmakePresets.json to point to vcpkg
if ("$ENV{VCPKG_ROOT}" STREQUAL "")
message(FATAL_ERROR "VCPKG_ROOT not found")
endif()

project(grpc-bridge VERSION 0.0.1 LANGUAGES CXX)

message(STATUS "vcpkg installed dir: ${VCPKG_INSTALLED_DIR}")

option(USE_LOCAL_GRPC   "Use local grpc source" OFF)
# set(USE_LOCAL_GRPC ON)
if(USE_LOCAL_GRPC)
  set(gRPC_INSTALL_default OFF)
  set(protobuf_INSTALL OFF)
  set(ABSL_ENABLE_INSTALL OFF)
  set(utf8_range_ENABLE_INSTALL OFF)
  # Try use the same cache
  add_subdirectory(../grpc ${CMAKE_CURRENT_SOURCE_DIR}/../grpc/cmake/build EXCLUDE_FROM_ALL)
  #find_package(Protobuf REQUIRED)
  include(FindProtobuf)
  add_library(gRPC::grpc ALIAS grpc)
  add_library(gRPC::grpc++ ALIAS grpc++)
else()
  # use vcpkg
  find_package(Protobuf REQUIRED)
  find_package(gRPC CONFIG REQUIRED)
endif()

# Cmake find modules
#list(APPEND CMAKE_MODULE_PATH "${VCPKG_INSTALLED_DIR}/share/grpc")

add_subdirectory(examples)