# Vulkan loader shipped with the Windows app

`vulkan-1.dll` is the Khronos Vulkan loader 1.4.363.1 as LunarG signs and
ships it (Vulkan SDK and Vulkan Runtime 1.4.363.1), licensed Apache-2.0
(<https://github.com/KhronosGroup/Vulkan-Loader>). SHA-256
`f4740173f8706ee968cbf5611246877b55ce9ec1027bccd6aa3a632154378a5b`.

The Windows build links whisper.cpp's Vulkan backend, so the executable needs
`vulkan-1.dll` to start. GPU drivers install it, but a PC without one does
not have it. `tauri.windows.conf.json` installs this copy next to the
executable, where Windows looks first. With no GPU driver the loader finds no
device and speech runs on the CPU.
