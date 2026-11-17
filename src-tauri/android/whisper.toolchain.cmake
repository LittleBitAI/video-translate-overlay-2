# whisper.cpp for Android, built by whisper-rs-sys through the cmake crate
# (.cargo/config.toml points cross builds here). The NDK's own toolchain file
# does the work; this picks its ABI from the Rust target cargo gives the build
# script (`TARGET`, inherited by cmake).

if("$ENV{TARGET}" MATCHES "^aarch64")
  set(ANDROID_ABI arm64-v8a)
  # ARMv8.2 dot product and half precision: about 30 % faster inference on a
  # Galaxy S21+ (docs/design/transcription-speed.md). A phone without them is
  # refused before the model loads (`cpu_runs_model` in src/whisper.rs).
  set(GGML_CPU_ARM_ARCH "armv8.2-a+dotprod+fp16" CACHE STRING "" FORCE)
elseif("$ENV{TARGET}" MATCHES "^x86_64")
  set(ANDROID_ABI x86_64)
elseif("$ENV{TARGET}" MATCHES "^i686")
  set(ANDROID_ABI x86)
else()
  set(ANDROID_ABI armeabi-v7a)
endif()
set(ANDROID_PLATFORM android-24)
include("$ENV{NDK_HOME}/build/cmake/android.toolchain.cmake")

# whisper-rs-sys tests the host OS, not the target, and adds MSVC's /utf-8,
# which the NDK's clang would read as a file name.
string(REPLACE "/utf-8" "" CMAKE_CXX_FLAGS "${CMAKE_CXX_FLAGS}")
