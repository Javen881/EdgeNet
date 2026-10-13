cmake_minimum_required(VERSION 3.22)

project(
    edgenet_vpn_bridge
    LANGUAGES C CXX
)

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_CXX_EXTENSIONS OFF)

set(
    OPENVPN3_DIR
    "${CMAKE_CURRENT_LIST_DIR}/../openvpn3"
    CACHE PATH
    "OpenVPN3 source directory"
)

if(NOT EXISTS "${OPENVPN3_DIR}/client/ovpncli.hpp")
    message(
        FATAL_ERROR
        "OpenVPN3 not found: ${OPENVPN3_DIR}"
    )
endif()

message(STATUS "OpenVPN3 source: ${OPENVPN3_DIR}")

find_package(OpenSSL REQUIRED)
find_package(asio CONFIG REQUIRED)
find_package(fmt CONFIG REQUIRED)
find_package(lz4 CONFIG REQUIRED)
find_package(jsoncpp CONFIG REQUIRED)
find_path(TAP_WINDOWS_INCLUDE_DIR tap-windows.h REQUIRED)

add_library(
    edgenet_vpn
    STATIC
    tap_manager.cpp
    wintun_manager.cpp
    edgenet_vpn.cpp
    "${OPENVPN3_DIR}/client/ovpncli.cpp"
    "${OPENVPN3_DIR}/openvpn/crypto/data_epoch.cpp"
)

target_include_directories(
    edgenet_vpn

    PUBLIC
        "${CMAKE_CURRENT_LIST_DIR}"

    PRIVATE
        "${OPENVPN3_DIR}"
        "${TAP_WINDOWS_INCLUDE_DIR}"
)

target_compile_definitions(
    edgenet_vpn

    PRIVATE
        USE_ASIO
        USE_OPENSSL
        EDGENET_VPN_BUILD	
        OPENVPN_PLATFORM_WIN
	TAP_WIN_COMPONENT_ID=tap0901
        ASIO_STANDALONE
        WIN32_LEAN_AND_MEAN
        NOMINMAX
        _WIN32_WINNT=0x0601
        NTDDI_VERSION=0x06010000
)

target_link_libraries(
    	edgenet_vpn

    PRIVATE
        OpenSSL::SSL
        OpenSSL::Crypto

        asio::asio
        fmt::fmt
        lz4::lz4
        JsonCpp::JsonCpp

        ws2_32
        iphlpapi
        crypt32
        bcrypt
        wininet
        setupapi
        rpcrt4
        wtsapi32
        ncrypt
        fwpuclnt
        ole32
        shell32
)

if(MINGW)
    target_compile_options(
        edgenet_vpn
        PRIVATE
            -Wall
            -Wextra
    )
endif()

set_target_properties(
    edgenet_vpn

    PROPERTIES

        ARCHIVE_OUTPUT_DIRECTORY
            "${CMAKE_BINARY_DIR}"

        LIBRARY_OUTPUT_DIRECTORY
            "${CMAKE_BINARY_DIR}"

        RUNTIME_OUTPUT_DIRECTORY
            "${CMAKE_BINARY_DIR}"
)

message(STATUS "EdgeNet VPN Bridge configured")
