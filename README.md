# AHG Universal Converter

Modern all-in-one desktop media downloader :

AHG Universal Converter provides a fast, polished, and portable experience for downloading music and video from multiple platforms through a single unified interface — without requiring accounts, subscriptions, or API credentials.

---

## Features

- Multi-platform media downloading
- YouTube search + direct URL support
- Spotify tracks, albums, and playlists
- SoundCloud tracks, playlists, and sets
- Batch URL importing with automatic platform detection
- Video downloads up to 4K
- Multiple audio formats:
  - MP3
  - FLAC
  - AAC
  - OGG
  - WAV
  - M4A
- Real-time progress, speed, and ETA tracking
- Drag-to-reorder download queue
- Persistent download history
- Metadata and thumbnail embedding
- Configurable output and quality settings
- Intelligent fallback download system
- Modern animated UI/UX
- No installation required

---

## Supported Platforms

 Platform Support

- YouTube: Videos, playlists, shorts, audio
- Spotify: Tracks, albums, playlists 
- SoundCloud: Tracks, playlists, sets 

---

## Core Sections

### YouTube
- Paste URL or search directly
- Automatic content type detection
- Download video or audio formats

### Spotify
- Paste track, album, or playlist links
- Full metadata + album artwork support
- No API credentials required

### SoundCloud
- URL or search support
- Playlist and metadata support

### Batch Import
- Paste multiple URLs at once
- Automatic platform detection per line

### Downloads
- Real-time download monitoring
- Speed and ETA tracking
- Queue reordering
- Persistent download history

### Settings
- Output folder selection
- Format and quality configuration
- Metadata + thumbnail embedding
- Speed limiter controls
- UI visual customization options

---

## Design Philosophy

- Dark-only interface optimized for low-light environments
- OKLCH-based warm color palette
- Plus Jakarta Sans Variable typography
- Smooth motion system powered by Framer Motion
- Premium desktop-app inspired UI/UX

---

## Tech Stack

- Electron 33
- React 18
- TypeScript
- Vite
- Tailwind CSS 4
- Framer Motion
- Zustand
- Lucide React
- electron-builder

---

## Under the Hood

- Bundled `yt-dlp` + `FFmpeg`
- Multi-layer Spotify metadata scraping
- Intelligent fallback architecture:
  - votify
  - spotDL
  - yt-dlp
- GitHub-based license activation system


---

## Distribution

No installer required — download and run.

---

## Current Version

`v0.3.4`

## License

MIT
