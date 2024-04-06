# Copyright 2021 the gRPC authors.
#
# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#
#     http://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing, software
# distributed under the License is distributed on an "AS IS" BASIS,
# WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
# See the License for the specific language governing permissions and
# limitations under the License.
#
# cmake build file for C++ keyvaluestore example.
# Assumes protobuf and gRPC have been installed using cmake.
# See cmake_externalproject/CMakeLists.txt for all-in-one cmake build
# that automatically builds all the dependencies before building keyvaluestore.

cmake_minimum_required(VERSION 3.8)

project(KeyValueStore C CXX)

# Proto file
get_filename_component(PROTOS "./keyvaluestore.proto" ABSOLUTE)
#get_filename_component(PROTOS "${kvs_proto}" NAME_WE)

# Generated sources
protobuf_generate_cpp(PROTO_SRCS PROTO_HDRS ${PROTOS})
protobuf_generate(LANGUAGE grpc
    PLUGIN "protoc-gen-grpc=${grpc_cpp_plugin_location}"
    OUT_VAR GRPC_SRCS
    APPEND_PATH
    GENERATE_EXTENSIONS
        .grpc.pb.h
        .grpc.pb.cc
    PROTOS ${PROTOS}
)

# Include generated *.pb.h files
include_directories("${CMAKE_CURRENT_BINARY_DIR}")

# kvs_grpc_proto
add_library(kvs_grpc_proto
  ${PROTO_SRCS}
  ${PROTO_HDRS}
  ${GRPC_SRCS}
)
target_link_libraries(kvs_grpc_proto
  PUBLIC
  gRPC::grpc
  gRPC::grpc++
  protobuf::libprotobuf
)

# client
add_executable(client "client.cc" "caching_interceptor.h")
target_link_libraries(client
  kvs_grpc_proto
  gRPC::grpc
  gRPC::grpc++
  protobuf::libprotobuf)

# server
add_executable(server "server.cc")
target_link_libraries(server
  kvs_grpc_proto
  gRPC::grpc
  gRPC::grpc++
  protobuf::libprotobuf)