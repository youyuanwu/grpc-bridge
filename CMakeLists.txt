cmake_minimum_required(VERSION 3.20)

# TODO: use vcpkg when grpc 1.63 releases
# configure vcpkg
if ("$ENV{VCPKG_ROOT}" STREQUAL "")
message(FATAL_ERROR "VCPKG_ROOT not found")
endif()
set(CMAKE_TOOLCHAIN_FILE "$ENV{VCPKG_ROOT}/scripts/buildsystems/vcpkg.cmake")
project(grpc-bridge VERSION 0.0.1 LANGUAGES CXX)

message(STATUS "vcpkg installed dir: ${VCPKG_INSTALLED_DIR}")

set(CMAKE_CXX_STANDARD 17)

option(USE_LOCAL_GRPC   "Use local grpc source" OFF)
option(USE_FETCH_CONTENT_GRPC   "Fetch grpc from github" ON)
# set(USE_LOCAL_GRPC ON) # use latest local grpc
if(USE_LOCAL_GRPC)
  set(gRPC_INSTALL_default OFF)
  set(protobuf_INSTALL OFF)
  set(ABSL_ENABLE_INSTALL OFF)
  set(utf8_range_ENABLE_INSTALL OFF)
  if(USE_FETCH_CONTENT_GRPC)
    message(STATUS "Fetching gRPC")
    include(FetchContent)
    FetchContent_Declare(
      grpc
      GIT_REPOSITORY https://github.com/grpc/grpc
      GIT_TAG        c18e8559e78af1a3bb356a6f1acc0baf262ecd73
    )
    FetchContent_GetProperties(grpc)
    if(NOT grpc_POPULATED)
        FetchContent_Populate(grpc)
        add_subdirectory(${grpc_SOURCE_DIR} ${grpc_BINARY_DIR} EXCLUDE_FROM_ALL)
    endif()
  else()
    # use grpc repo in relative dir.
    add_subdirectory(../grpc ${CMAKE_CURRENT_BINARY_DIR}/grpc_build EXCLUDE_FROM_ALL)
  endif()
  #find_package(Protobuf REQUIRED)
  include(FindProtobuf)
  add_library(gRPC::grpc ALIAS grpc)
  add_library(gRPC::grpc++ ALIAS grpc++)
  add_library(gRPC::grpc++_reflection ALIAS grpc++_reflection)
else()
  message(STATUS "using vcpkg")
  # use vcpkg
  find_package(Protobuf REQUIRED)
  find_package(gRPC CONFIG REQUIRED)
endif()

# Cmake find modules
#list(APPEND CMAKE_MODULE_PATH "${VCPKG_INSTALLED_DIR}/share/grpc")

add_subdirectory(examples)

find_program(dotnet_exe
  NAMES dotnet.exe dotnet
  REQUIRED
)

add_custom_target(build_dotnet ALL
  WORKING_DIRECTORY ${CMAKE_CURRENT_SOURCE_DIR}
  COMMAND ${dotnet_exe} build
)

# download symcrypt
set(release_url https://github.com/microsoft/SymCrypt/releases/download/v103.8.0/symcrypt-windows-amd64-release-103.8.0-53be637d.zip)

message(STATUS "fetching symcrypt_release " ${release_url})
include(FetchContent)
FetchContent_Declare(
  symcrypt_release
  DOWNLOAD_EXTRACT_TIMESTAMP true
  URL ${release_url}
)

FetchContent_GetProperties(symcrypt_release)
if(NOT symcrypt_release_POPULATED)
  FetchContent_Populate(symcrypt_release)
endif()

# copy symcrypt to rust dir
file(
  COPY ${symcrypt_release_SOURCE_DIR}/dll/symcrypt.dll
  DESTINATION ${CMAKE_CURRENT_SOURCE_DIR}/target/debug/deps
)

# generate test cert
if(WIN32)
# get the test certificate
execute_process(COMMAND pwsh.exe -Command "Get-ChildItem Cert:\\CurrentUser\\My | Where-Object -Property FriendlyName -EQ -Value YARRP-Test | Select-Object -ExpandProperty Thumbprint -First 1"
  OUTPUT_VARIABLE TEST_CERT_HASH
)

if(TEST_CERT_HASH STREQUAL "")
  message(Status "Generating TEST_CERT_HASH")
  execute_process(
    COMMAND pwsh.exe -Command "New-SelfSignedCertificate -DnsName $env:computername,localhost -FriendlyName YARRP-Test -KeyUsageProperty Sign -KeyUsage DigitalSignature -CertStoreLocation cert:\\CurrentUser\\My -HashAlgorithm SHA256 -Provider \"Microsoft Software Key Storage Provider\" -KeyExportPolicy Exportable"
    COMMAND_ERROR_IS_FATAL ANY
  )
  execute_process(COMMAND pwsh.exe -Command "Get-ChildItem Cert:\\CurrentUser\\My | Where-Object -Property FriendlyName -EQ -Value YARRP-Test | Select-Object -ExpandProperty Thumbprint -First 1"
    OUTPUT_VARIABLE TEST_CERT_HASH
    COMMAND_ERROR_IS_FATAL ANY
  )
endif()
message(STATUS "Using TEST_CERT_HASH ${TEST_CERT_HASH}")
endif(WIN32)