cmake_minimum_required(VERSION 3.20)

set(PROTOC_VERSION "34.0")
set(PROTOC_GEN_RUST_GRPC_VERSION "0.9.0")

if(NOT DEFINED GRPC_RUST_TOOLS_DIR)
  set(GRPC_RUST_TOOLS_DIR
      "${CMAKE_CURRENT_LIST_DIR}/../target/grpc-rust-tools")
endif()

string(TOLOWER "${CMAKE_HOST_SYSTEM_PROCESSOR}" host_processor)
if(host_processor STREQUAL "")
  if(CMAKE_HOST_SYSTEM_NAME STREQUAL "Windows")
    string(TOLOWER "$ENV{PROCESSOR_ARCHITECTURE}" host_processor)
  else()
    execute_process(
      COMMAND uname -m
      OUTPUT_VARIABLE host_processor
      OUTPUT_STRIP_TRAILING_WHITESPACE
      COMMAND_ERROR_IS_FATAL ANY)
    string(TOLOWER "${host_processor}" host_processor)
  endif()
endif()

if(CMAKE_HOST_SYSTEM_NAME STREQUAL "Windows")
  if(host_processor MATCHES "^(amd64|x86_64)$")
    set(protoc_platform "win64")
    set(protoc_sha256
        "76ddeb5ae7a31c8f9f7759d3b843a4cadda2150ac037ad0c1794665d6cf31fce")
    set(plugin_platform "win64")
    set(plugin_sha256
        "734e2ea947b32a85becdbf6c27b1353d5a499bb69fbf287446fb8c969fa6e679")
  endif()
  set(executable_suffix ".exe")
elseif(CMAKE_HOST_SYSTEM_NAME STREQUAL "Linux")
  if(host_processor MATCHES "^(amd64|x86_64)$")
    set(protoc_platform "linux-x86_64")
    set(protoc_sha256
        "e9a91b6fcfe4177ec2cd35fc8f15c1e811fa0ecdef9372755cd6d3513d5faaab")
    set(plugin_platform "linux-x86_64")
    set(plugin_sha256
        "c0e93891f39f8188c23d2026af972f07547066fb3c2973f26148f9b4d52c5320")
  elseif(host_processor MATCHES "^(aarch64|arm64)$")
    set(protoc_platform "linux-aarch_64")
    set(protoc_sha256
        "f0b8aad28be5ea6150c082f96ac57e028154afb9ee29f4ce092b5a39df8ae6c8")
    set(plugin_platform "linux-aarch_64")
    set(plugin_sha256
        "cfc711728b340ec62c43ea3905d5d626441438f017f8552be9c8384503ebfa3e")
  endif()
  set(executable_suffix "")
elseif(CMAKE_HOST_SYSTEM_NAME STREQUAL "Darwin")
  if(host_processor MATCHES "^(amd64|x86_64)$")
    set(protoc_platform "osx-x86_64")
    set(protoc_sha256
        "d58fcd413a9ed458283d54023e409fd5cf767da4ed225d1ffaffd83cf2764f53")
  elseif(host_processor MATCHES "^(aarch64|arm64)$")
    set(protoc_platform "osx-aarch_64")
    set(protoc_sha256
        "3ef35187a3c8aed81ee57e792227e483e558fa56c93fce525e569bff55794c1a")
  endif()
  set(plugin_platform "osx-universal_binary")
  set(plugin_sha256
      "6315f964a34e576d962fae6a1c2bcc747c07a01e1c89a1ea8570940075e39e0f")
  set(executable_suffix "")
endif()

if(NOT DEFINED protoc_platform OR NOT DEFINED plugin_platform)
  message(FATAL_ERROR
          "Unsupported host: ${CMAKE_HOST_SYSTEM_NAME}/${host_processor}")
endif()

function(download_and_extract name url sha256 executable)
  if(EXISTS "${executable}")
    message(STATUS "Using existing ${name}: ${executable}")
    return()
  endif()

  set(archive "${GRPC_RUST_TOOLS_DIR}/${name}.zip")
  message(STATUS "Downloading ${name} from ${url}")
  file(DOWNLOAD "${url}" "${archive}"
       EXPECTED_HASH "SHA256=${sha256}"
       STATUS download_status
       TLS_VERIFY ON)
  list(GET download_status 0 status_code)
  list(GET download_status 1 status_message)
  if(NOT status_code EQUAL 0)
    file(REMOVE "${archive}")
    message(FATAL_ERROR "Failed to download ${name}: ${status_message}")
  endif()

  file(ARCHIVE_EXTRACT INPUT "${archive}" DESTINATION "${GRPC_RUST_TOOLS_DIR}")
  file(REMOVE "${archive}")
  if(NOT EXISTS "${executable}")
    message(FATAL_ERROR "${name} archive did not contain ${executable}")
  endif()
endfunction()

file(MAKE_DIRECTORY "${GRPC_RUST_TOOLS_DIR}")
set(protoc "${GRPC_RUST_TOOLS_DIR}/bin/protoc${executable_suffix}")
set(plugin
    "${GRPC_RUST_TOOLS_DIR}/bin/protoc-gen-rust-grpc${executable_suffix}")

download_and_extract(
  "protoc"
  "https://github.com/protocolbuffers/protobuf/releases/download/v${PROTOC_VERSION}/protoc-${PROTOC_VERSION}-${protoc_platform}.zip"
  "${protoc_sha256}"
  "${protoc}")
download_and_extract(
  "protoc-gen-rust-grpc"
  "https://github.com/grpc/grpc-rust/releases/download/protoc-gen-rust-grpc-v${PROTOC_GEN_RUST_GRPC_VERSION}/protoc-gen-rust-grpc-${PROTOC_GEN_RUST_GRPC_VERSION}-${plugin_platform}.zip"
  "${plugin_sha256}"
  "${plugin}")

if(NOT CMAKE_HOST_SYSTEM_NAME STREQUAL "Windows")
  file(CHMOD "${protoc}" "${plugin}"
       PERMISSIONS OWNER_READ OWNER_WRITE OWNER_EXECUTE
                   GROUP_READ GROUP_EXECUTE
                   WORLD_READ WORLD_EXECUTE)
endif()

message(STATUS "gRPC Rust tools installed in ${GRPC_RUST_TOOLS_DIR}/bin")
