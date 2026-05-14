# AHG Universal Converter

Modern all-in-one desktop media downloader :

AHG Universal Converter provides a fast, polished, and portable experience for downloading music and video from multiple platforms through a single unified interface — without requiring accounts, subscriptions, or API credentials.

---

## Features:

- Multi-platform media downloading:
<img width="1148" height="690" alt="image" src="https://github.com/user-attachments/assets/ef1dec23-d5e6-4f9c-8ebf-b15b8965e20f" />

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
- Real-time progress, speed, and ETA tracking:
- Drag-to-reorder download queue
- Persistent download history
- Metadata and thumbnail embedding
- Configurable output and quality settings
- Intelligent fallback download system
- Modern animated UI/UX
- No installation required

---

## Supported Platforms

•  <img src="https://cdn.simpleicons.org/youtube/FF0000" width="18" align="center" /> **YouTube** : Videos, playlists, shorts, audio  
•  <img src="https://cdn.simpleicons.org/spotify/1DB954" width="18" align="center" /> **Spotify** : Tracks, albums, playlists  
•  <img src="https://cdn.simpleicons.org/soundcloud/FF5500" width="18" align="center" /> **SoundCloud** : Tracks, playlists, sets

---


## Core Sections

### YouTube:
<img width="1868" height="896" alt="image" src="https://github.com/user-attachments/assets/5f2daaf9-da35-48c8-a77b-715e3accb437" />

- Paste URL or search directly
- Automatic content type detection
- Download video or audio formats

### Spotify:
<img width="1859" height="945" alt="image" src="https://github.com/user-attachments/assets/c193adc2-2bb5-4791-af4f-088881f4b52f" />
- Paste track, album, or playlist links
- Full metadata + album artwork support:
  <img width="1920" height="2560" alt="image" src="https://github.com/user-attachments/assets/a34afd2a-6299-4dc9-97eb-af32195f0af8" />

- No API credentials required

### SoundCloud
- URL or search support:
  <img width="1849" height="917" alt="image" src="https://github.com/user-attachments/assets/c50fddbf-4715-4421-bb68-b33f59f3e5bd" />

- Playlist and metadata support

### Batch Import
- Paste multiple URLs at once:
  <img width="1656" height="991" alt="image" src="https://github.com/user-attachments/assets/56b76bd0-c969-46aa-9c4b-281684fb2704" />

- Automatic platform detection per line

### Downloads:
<img width="498" height="156" alt="converted_animation" src="https://github.com/user-attachments/assets/dc2a7814-f070-4c94-aae5-fbf0a1a810e9" />

- Real-time download monitoring
- Speed and ETA tracking
- Queue reordering
- Persistent download history

### Settings: 
<img width="1919" height="1026" alt="image" src="https://github.com/user-attachments/assets/29238975-5615-4c23-a463-265f6ad2dbb1" />

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
