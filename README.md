# Commander 🎤

A macOS-only speech-to-text transcription app that lives in your menu bar with a global shortcut for hands-free capture. Audio never leaves your device.

![Commander App Icon](src-tauri/icons/commander-256x256.png)

## Features

- 🎯 **Custom Global Shortcuts**: Start/stop recording from anywhere
- 🔒 **Local Processing**: Whisper runs fully on-device for privacy
- 📋 **Auto-Clipboard**: Transcriptions are copied automatically
- 🖥️ **Menu Bar App**: Lightweight background app with clear status icons

## Quick Start

1. Launch Commander
2. Open the main window and configure your global shortcut
3. Press the shortcut to start recording
4. Press again to stop and transcribe
5. Paste anywhere — the text is already in your clipboard

## Installation

### Requirements

- macOS 11.0+
- Rust (latest stable)
- Node.js and pnpm
- Microphone permission (requested on first record)

### Build from source

```bash
# Install dependencies
pnpm install

# Run in development
pnpm tauri dev

# Build a production app bundle (DMG)
pnpm tauri build
```

## Usage

- Configure your preferred global shortcut in the main window
- Use the shortcut to toggle recording from anywhere in macOS
- Watch the menu bar icon change to confirm status (idle → recording → transcribing)
- After transcription, paste the text where you need it

## Development

```bash
# Install deps
pnpm install

# Start the app with live reload
pnpm tauri dev
```

### End-to-end tests (Playwright)

```bash
# One-time: install browsers
pnpm exec playwright install


# Make sure the dev server is running
pnpm dev

# Run all tests
pnpm playwright test

# Run a specific test file
pnpm playwright test tests/tray.spec.ts
```

## Privacy

- Audio is captured and processed locally on your Mac.
- No audio or text leaves your device during transcription.

## macOS Permissions

Commander requires Microphone and Accessibility permissions:

- Microphone: requested on first record; needed to capture audio for transcription
- Accessibility: required to detect a hold-down global keyboard shortcut and to perform automatic paste

To (re-)enable:

System Settings → Privacy & Security → Microphone → enable for Commander
System Settings → Privacy & Security → Accessibility → enable for Commander

## Troubleshooting

- If the shortcut doesn’t trigger, ensure Commander has Accessibility permissions (System Settings → Privacy & Security → Accessibility)
- If audio isn’t recorded, confirm Microphone permission is granted


## Disclaimer

> [!CAUTION]
> This application is built for personal use and is not intended for production environments. Use at your own risk.