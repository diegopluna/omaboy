cmake_minimum_required(VERSION 3.21)
project(omaboy VERSION 0.3.0 LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
if(NOT CMAKE_BUILD_TYPE)
    set(CMAKE_BUILD_TYPE Release)
endif()

find_package(Qt6 6.5 REQUIRED COMPONENTS Quick Multimedia)
qt_standard_project_setup(REQUIRES 6.5)
qt_policy(SET QTP0004 NEW)

# ---- Rust core (staticlib via cargo) ----
set(CORE_DIR ${CMAKE_CURRENT_SOURCE_DIR}/core)
set(CORE_LIB ${CORE_DIR}/target/release/libomaboy_core.a)

file(GLOB_RECURSE CORE_SOURCES ${CORE_DIR}/src/*.rs ${CORE_DIR}/Cargo.toml)
add_custom_command(
    OUTPUT ${CORE_LIB}
    COMMAND cargo build --release
    WORKING_DIRECTORY ${CORE_DIR}
    DEPENDS ${CORE_SOURCES}
    COMMENT "Building Rust emulator core"
    VERBATIM
)
add_custom_target(core_build DEPENDS ${CORE_LIB})

add_library(omaboy_core STATIC IMPORTED)
set_target_properties(omaboy_core PROPERTIES IMPORTED_LOCATION ${CORE_LIB})
add_dependencies(omaboy_core core_build)

# ---- Qt application ----
qt_add_executable(omaboy
    src/main.cpp
    src/emulator.cpp
    src/emulator.h
    src/inputmap.cpp
    src/inputmap.h
    src/screenitem.cpp
    src/screenitem.h
    src/theme.cpp
    src/theme.h
)

qt_add_qml_module(omaboy
    URI Omaboy
    VERSION 1.0
    QML_FILES
        qml/Main.qml
        qml/StatusBar.qml
        qml/RomBrowser.qml
        qml/HelpOverlay.qml
        qml/SettingsOverlay.qml
        qml/Toast.qml
)

target_include_directories(omaboy PRIVATE src)
target_compile_definitions(omaboy PRIVATE OMABOY_VERSION="${PROJECT_VERSION}")
target_link_libraries(omaboy PRIVATE
    Qt6::Quick
    Qt6::Multimedia
    omaboy_core
    pthread dl m
)

install(TARGETS omaboy RUNTIME DESTINATION bin)
install(FILES assets/omaboy.desktop DESTINATION share/applications)
install(FILES assets/omaboy.svg DESTINATION share/icons/hicolor/scalable/apps)
