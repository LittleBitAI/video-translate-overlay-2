# Desktop transcription speed

On 2026-11-17 the Windows app (0.1.8) took 2 h 14 min to write and translate
a 23-minute English video (YouTube `WJpeW9lCSUM`, 53 windows) on an
i7-12700F with an RTX 3080. The user had expected the PC to be faster than
the phone; it was slower than real time.

## Why it was slow

- whisper.cpp was built for the CPU only. The GPU did nothing.
- The desktop decoded `large-v3-turbo q5_0` with beam search 5 and up to five
  temperature retries per window, on 8 threads. A 30-second window took about
  2.5 minutes.

## What changed

- Windows builds whisper.cpp with its Vulkan backend (`whisper-rs` feature
  `vulkan`), which runs on NVIDIA, AMD and Intel GPUs through their drivers.
  `whisper::gpu` reports the device whisper.cpp takes; the app logs
  `speech model on <device>`.
- With a GPU: beam search 5 with temperature retries, as before, and flash
  attention off.
- Without a GPU (no device, or no Vulkan driver): the phone's decoding,
  greedy with no temperature retries and a second decoding only for a window
  that loops, still with `large-v3-turbo` and up to 8 threads.
- The installer ships the Vulkan loader next to the executable
  (`src-tauri/windows/README.md`), so a PC without a GPU driver still starts
  the app and writes on the CPU.

## Measurements

`transcribe_timing` on the same audio, RTX 3080, Vulkan, `large-v3-turbo q5_0`.
First 5 minutes (12 windows, 317 s of speech):

| Decoding | Infer time | Per window |
|---|---|---|
| beam 5, flash attention, retries | 500 s | 0.75-94 s |
| beam 5, flash attention, no retries | 288 s | 0.75-74 s |
| beam 5, no flash attention, retries | 14.8 s | 6 s first, then 0.75-1 s |
| greedy, flash attention | 5.5 s | 0.25-0.5 s |

Flash attention with beam search on Vulkan is the slow case and stays off.
Beam search and greedy agreed on 99.4 % of the words. Beam search wrote
"Mamba" where greedy and the old CPU decoding wrote "member", so the GPU keeps
beam search.

The whole video with the shipped settings: 53 windows, 1,418 s of speech,
73.5 s of inference (slowest window 8.8 s). Before, the app's whole job
(writing and translation) took 2 h 14 min; translation is not part of this
measurement.

With the Vulkan driver hidden (`VK_DRIVER_FILES` pointing nowhere) and the
shipped loader, the harness ran on the CPU without error. Its CPU speed was
not measured: other programs were using 58-68 % of the CPU and most of the
memory at the time, and one 26-second window took 94.6 s.

## Building

The Windows build needs the Vulkan SDK (`VULKAN_SDK`: headers, `glslc`,
`vulkan-1.lib`), 1.4.363.1 here. Android and the default `tools` build
(CI's gate) do not compile whisper.cpp with Vulkan. CMake inside
whisper-rs-sys fails to detect the compiler under a very long target path;
a short `CARGO_TARGET_DIR` avoids it.
