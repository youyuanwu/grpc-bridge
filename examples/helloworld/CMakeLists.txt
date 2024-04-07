set(PROTOS
    ${CMAKE_CURRENT_SOURCE_DIR}/helloworld.proto
)

set(PROTO_SRC_DIR ${CMAKE_CURRENT_BINARY_DIR})
file(MAKE_DIRECTORY ${PROTO_SRC_DIR})
include_directories(${PROTO_SRC_DIR})

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

# Building server
add_executable(greeter_server
    ${CMAKE_CURRENT_SOURCE_DIR}/greeter_server.cc
    ${PROTO_SRCS}
    ${GRPC_SRCS}
)
target_link_libraries(greeter_server
    gRPC::grpc++_reflection
    protobuf::libprotobuf
)

# Building client
add_executable(greeter_client
    ${CMAKE_CURRENT_SOURCE_DIR}/greeter_client.cc
    ${PROTO_SRCS}
    ${GRPC_SRCS}
)
target_link_libraries(greeter_client
    gRPC::grpc++_reflection
    protobuf::libprotobuf
)