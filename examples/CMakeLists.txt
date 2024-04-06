if(USE_LOCAL_GRPC)
  set(grpc_cpp_plugin_location "$<TARGET_FILE:grpc_cpp_plugin>")
else()
  get_target_property(grpc_cpp_plugin_location gRPC::grpc_cpp_plugin LOCATION)  
endif()

add_subdirectory(helloworld)
add_subdirectory(interceptors)