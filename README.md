# Volta Wave GTK

A native GTK4 music player with audio visualizations, built with Rust. Part of the Volta Agent ecosystem.

## Features

- **Audio Playback**: Play MP3, FLAC, OGG, WAV, M4A, AAC, and WebM files
- **6 Visualization Modes**: Bars, Wave, Circles, Stars, Mirror, Spectrum
- **8 Color Themes**: Tokyo Night, Gruvbox, Dracula, Nord, Catppuccin, Solarized, Cyberpunk, Forest
- **Shuffle & Repeat**: Randomize playback or repeat single/all tracks
- **Lyrics Support**: Automatic lyrics fetching from LRCLIB
- **Keyboard Shortcuts**: Space (play/pause), Left/Right arrows (seek +/-5s), A/D (seek)
- **Search**: Filter tracks by title or artist
- **Queue Management**: View and manage play queue

## Screenshots

```
┌─────────────────────────────────────────────────────────────────┐
│  Volta Wave                    [Theme ▼]                        │
├───────────────┬─────────────────────────────────────────────────┤
│ [Library][Queue]                                                │
│ ┌───────────┐ │  ┌────────────────────────────────────────────┐ │
│ │ Search... │ │  │                                            │ │
│ └───────────┘ │  │        ████  ████████  ████████            │ │
│  42 tracks    │  │      ██████  ██████████  ████████          │ │
│               │  │     ███████  █████████████  ████████       │ │
│ ┌───────────┐ │  │    ████████  ██████████████  ██████████    │ │
│ │ Artist    │ │  │   █████████  ████████████████  ████████████│ │
│ │  Title    │ │  │                                            │ │
│ │ Artist2   │ │  └────────────────────────────────────────────┘ │
│ │  Title2   │ │                                                │
│ │ ...       │ │  ◀ ◀    ▶    ▶ ▶                               │
│ └───────────┘ │                                                │
│               │  0:00 ━━━━━━━━━━━━━━━━━━━━━ 4:32    🔊 ────    │
│               │                                                │
│               │  Tokyo Night                         Bars      │
└───────────────┴─────────────────────────────────────────────────┘
```

## Installation

### Prerequisites

- Rust 1.70+
- GTK4 development libraries
- GStreamer plugins

```bash
# Ubuntu/Debian
sudo apt install libgtk-4-dev libgstreamer1.0-dev gstreamer1.0-plugins-base gstreamer1.0-plugins-good gstreamer1.0-plugins-bad

# Arch Linux
sudo pacman -S gtk4 gstreamer gst-plugins-base gst-plugins-good gst-plugins-bad

# Fedora
sudo dnf install gtk4-devel gstreamer1-devel gstreamer1-plugins-base-devel
```

### Build

```bash
cd ~/projects/volta-wave-gtk
cargo build --release
```

The binary will be at `target/release/volta-wave-gtk`.

### Install Desktop Entry

```bash
cp volta-wave-gtk.desktop ~/.local/share/applications/
update-desktop-database ~/.local/share/applications/
```

## Usage

### Running

```bash
./target/release/volta-wave-gtk
```

Or launch from your desktop menu under "Audio" category.

### Music Library

The player scans `~/Music` directory recursively for audio files. Files should be named:
- `Artist - Title.mp3` (recommended)
- Or just `Title.mp3` (shows as Unknown artist)

### Keyboard Shortcuts

| Key | Action |
|-----|--------|
| Space | Play/Pause |
| Left Arrow / A | Seek backward 5 seconds |
| Right Arrow / D | Seek forward 5 seconds |

### Lyrics

Lyrics are fetched automatically from [LRCLIB](https://lrclib.net/) based on artist and title. You can also provide `.lrc` files alongside your music files:

```
Artist - Title.mp3
Artist - Title.lrc   # Synced lyrics
```

## Configuration

- **Music Directory**: `~/Music` (edit `MUSIC_DIR` constant in `main.rs` to change)
- **Volume**: Persisted during session, defaults to 75%

## Dependencies

- **gtk4**: GUI toolkit
- **gstreamer**: Audio playback pipeline
- **glib**: Main event loop and utilities
- **cairo**: Visualization rendering
- **serde**: JSON parsing for lyrics API
- **reqwest**: HTTP client for lyrics fetching
- **walkdir**: Recursive directory scanning
- **regex**: LRC file parsing
- **rand**: Shuffle functionality

## Architecture

```
main.rs
├── UI Construction (build_ui)
│   ├── Sidebar (track list, search, queue)
│   ├── Visualization (DrawingArea)
│   ├── Controls (play/pause, next, prev)
│   └── Status Bar
├── Playback (GStreamer playbin)
├── Visualization (6 modes)
├── Theme System (8 themes)
└── Lyrics Fetching (async)
```

## Related Projects

- **volta-wave-gui**: Web-based music player (port 3006)
- **volta-radio**: Internet radio player (port 3005)
- **volta-journal**: Journal API and web app

## License

MIT License - Part of the Volta Agent project.
