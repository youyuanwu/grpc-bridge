set(_fbs_file ../greeter.fbs)
set(_fbs_cpp_files 
  ${CMAKE_CURRENT_BINARY_DIR}/greeter_generated.h 
  ${CMAKE_CURRENT_BINARY_DIR}/greeter.grpc.fb.cc
  ${CMAKE_CURRENT_BINARY_DIR}/greeter.grpc.fb.h)
add_custom_command(
  OUTPUT ${_fbs_cpp_files}
  COMMAND flatc --grpc --cpp -o ${CMAKE_CURRENT_BINARY_DIR} ${CMAKE_CURRENT_SOURCE_DIR}/${_fbs_file}
  DEPENDS ${CMAKE_CURRENT_SOURCE_DIR}/${_fbs_file}
)

find_package(flatbuffers CONFIG REQUIRED)

add_executable(fbs_greeter_server
  server.cpp
  ${_fbs_cpp_files}
)

target_include_directories(
  fbs_greeter_server
  PRIVATE ${CMAKE_CURRENT_BINARY_DIR}
)

target_link_libraries(fbs_greeter_server
 PRIVATE flatbuffers::flatbuffers gRPC::grpc++_reflection
)

add_executable(fbs_greeter_client
  client.cpp
  ${_fbs_cpp_files}
)
target_include_directories(
  fbs_greeter_client
  PRIVATE ${CMAKE_CURRENT_BINARY_DIR}
)

target_link_libraries(fbs_greeter_client
  PRIVATE flatbuffers::flatbuffers gRPC::grpc++_reflection
)