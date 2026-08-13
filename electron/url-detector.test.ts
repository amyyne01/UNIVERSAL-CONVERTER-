import { describe, it, expect } from 'vitest';
import { detectUrl, parseSpotifyUrl } from './url-detector';

describe('detectUrl — YouTube', () => {
  it('recognises a watch?v= video and extracts the 11-char id', () => {
    const d = detectUrl('https://www.youtube.com/watch?v=dQw4w9WgXcQ');
    expect(d.platform).toBe('youtube');
    expect(d.contentType).toBe('video');
    expect(d.id).toBe('dQw4w9WgXcQ');
    expect(d.isCollection).toBe(false);
    expect(d.label).toBe('YouTube video');
  });

  it('recognises youtu.be, /shorts/, /embed/ and /live/ forms', () => {
    expect(detectUrl('https://youtu.be/dQw4w9WgXcQ').id).toBe('dQw4w9WgXcQ');
    expect(detectUrl('https://youtube.com/shorts/dQw4w9WgXcQ').contentType).toBe('video');
    expect(detectUrl('https://www.youtube.com/embed/dQw4w9WgXcQ').platform).toBe('youtube');
    const live = detectUrl('https://www.youtube.com/live/dQw4w9WgXcQ');
    expect(live.platform).toBe('youtube');
    expect(live.contentType).toBe('video');
    expect(live.id).toBe('dQw4w9WgXcQ');
  });

  it('treats a watch URL carrying a list= as a video, not a playlist', () => {
    const d = detectUrl('https://www.youtube.com/watch?v=dQw4w9WgXcQ&list=PLabc');
    expect(d.contentType).toBe('video');
    expect(d.id).toBe('dQw4w9WgXcQ');
  });

  it('recognises a /playlist?list= collection', () => {
    const d = detectUrl('https://www.youtube.com/playlist?list=PLabcDEF123');
    expect(d.platform).toBe('youtube');
    expect(d.contentType).toBe('playlist');
    expect(d.id).toBe('PLabcDEF123');
    expect(d.isCollection).toBe(true);
  });
});

describe('detectUrl — Spotify', () => {
  it('recognises the web notation', () => {
    const d = detectUrl('https://open.spotify.com/track/6rqhFgbbKwnb9MLmUQDhG6');
    expect(d.platform).toBe('spotify');
    expect(d.contentType).toBe('track');
    expect(d.id).toBe('6rqhFgbbKwnb9MLmUQDhG6');
  });

  it('recognises the uri notation', () => {
    const d = detectUrl('spotify:track:6rqhFgbbKwnb9MLmUQDhG6');
    expect(d.platform).toBe('spotify');
    expect(d.contentType).toBe('track');
    expect(d.id).toBe('6rqhFgbbKwnb9MLmUQDhG6');
  });

  it('tolerates the optional intl-xx segment', () => {
    const d = detectUrl('https://open.spotify.com/intl-fr/album/1DFixLWuPkv3KT3TnV35m3');
    expect(d.platform).toBe('spotify');
    expect(d.contentType).toBe('album');
    expect(d.id).toBe('1DFixLWuPkv3KT3TnV35m3');
    expect(d.isCollection).toBe(true);
  });

  it('marks playlists/albums as collections', () => {
    expect(detectUrl('https://open.spotify.com/playlist/37i9dQZF1DXcBWIGoYBM5M').isCollection).toBe(true);
    expect(detectUrl('https://open.spotify.com/artist/0OdUWJ0sBjDrqHygGUXeCF').isCollection).toBe(false);
  });
});

describe('parseSpotifyUrl', () => {
  it('parses both notations', () => {
    expect(parseSpotifyUrl('https://open.spotify.com/playlist/abc123')).toEqual({ type: 'playlist', id: 'abc123' });
    expect(parseSpotifyUrl('spotify:artist:xyz789')).toEqual({ type: 'artist', id: 'xyz789' });
    expect(parseSpotifyUrl('https://open.spotify.com/intl-pt/track/qwe456')).toEqual({ type: 'track', id: 'qwe456' });
  });

  it('returns nulls for a non-Spotify url', () => {
    expect(parseSpotifyUrl('https://youtube.com/watch?v=dQw4w9WgXcQ')).toEqual({ type: null, id: null });
  });
});

describe('detectUrl — SoundCloud', () => {
  it('recognises a track', () => {
    const d = detectUrl('https://soundcloud.com/artist-name/some-track');
    expect(d.platform).toBe('soundcloud');
    expect(d.contentType).toBe('track');
    expect(d.id).toBe('some-track');
  });

  it('recognises a /sets/ playlist as a collection', () => {
    const d = detectUrl('https://soundcloud.com/artist-name/sets/my-playlist');
    expect(d.platform).toBe('soundcloud');
    expect(d.contentType).toBe('playlist');
    expect(d.isCollection).toBe(true);
  });

  it('excludes profile sub-pages like /likes', () => {
    expect(detectUrl('https://soundcloud.com/artist-name/likes').platform).toBe('unknown');
    expect(detectUrl('https://soundcloud.com/artist-name/reposts').platform).toBe('unknown');
    expect(detectUrl('https://soundcloud.com/artist-name/followers').platform).toBe('unknown');
  });
});

describe('detectUrl — short-form video platforms', () => {
  it('recognises Instagram', () => {
    expect(detectUrl('https://www.instagram.com/reel/Cabc123/').platform).toBe('instagram');
  });
  it('recognises TikTok', () => {
    expect(detectUrl('https://www.tiktok.com/@user/video/1234567890').platform).toBe('tiktok');
  });
  it('recognises Facebook', () => {
    expect(detectUrl('https://www.facebook.com/watch?v=1234567890').platform).toBe('facebook');
  });
});

describe('detectUrl — fallbacks', () => {
  it('recognises a direct media file by extension', () => {
    const d = detectUrl('https://cdn.example.com/clips/video.mp4');
    expect(d.platform).toBe('direct');
    expect(d.contentType).toBe('video');
    expect(d.label).toBe('Direct media');
  });

  it('reports unknown for anything unrecognised', () => {
    const d = detectUrl('https://example.com/some/page');
    expect(d.platform).toBe('unknown');
    expect(d.label).toBe('Unknown link');
  });
});

describe('detectUrl — normalisation', () => {
  it('trims and prepends https:// when no scheme is present', () => {
    const d = detectUrl('  youtube.com/watch?v=dQw4w9WgXcQ  ');
    expect(d.url).toBe('https://youtube.com/watch?v=dQw4w9WgXcQ');
    expect(d.platform).toBe('youtube');
  });
});
