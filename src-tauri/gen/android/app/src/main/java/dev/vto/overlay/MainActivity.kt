package dev.vto.overlay

// No edge-to-edge: the settings screen sits below the status bar on Android 14
// and earlier (the S21 Ultra's last); later versions use the page's safe-area padding.
class MainActivity : TauriActivity()
