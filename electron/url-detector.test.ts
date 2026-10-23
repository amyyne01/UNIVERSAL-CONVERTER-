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

  // The point of the negative lookahead is that these are never mistaken for a
  // TRACK with a bogus slug id. They now fall through to the generic rule instead
  // of dying as 'unknown' — the engine does support /likes and /reposts — but the
  // thing being guarded against, a wrong soundcloud track detection, still holds.
  it('never claims profile sub-pages like /likes as tracks', () => {
    for (const sub of ['likes', 'reposts', 'followers']) {
      const d = detectUrl(`https://soundcloud.com/artist-name/${sub}`);
      expect(d.platform).not.toBe('soundcloud');
      expect(d.contentType).not.toBe('track');
    }
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

  it('hands any other real address to the engine as generic', () => {
    const d = detectUrl('https://example.org/watch/123456789');
    expect(d.platform).toBe('generic');
    expect(d.contentType).toBe('video');
    // The hostname names the site far better than the word "Link" does.
    expect(d.label).toBe('example.org video');
  });

  it('still reports unknown for input that is not a real address', () => {
    // detectUrl() prepends https:// to scheme-less input, so a typed search
    // phrase must NOT become a generic link and get sent to a spawn.
    for (const input of ['best songs of 2026', 'not a url', 'spotify:weird:thing']) {
      expect(detectUrl(input).platform).toBe('unknown');
    }
  });

  it('does not treat a non-http scheme as a generic link', () => {
    expect(detectUrl('file:///C:/Windows/System32/notes.txt').platform).toBe('unknown');
    expect(detectUrl('javascript:alert(1)').platform).toBe('unknown');
  });
});

describe('detectUrl — normalisation', () => {
  it('trims and prepends https:// when no scheme is present', () => {
    const d = detectUrl('  youtube.com/watch?v=dQw4w9WgXcQ  ');
    expect(d.url).toBe('https://youtube.com/watch?v=dQw4w9WgXcQ');
    expect(d.platform).toBe('youtube');
  });
});

describe('detectUrl — non-core sources', () => {
  const cases: Array<[string, string, string, string | undefined, boolean]> = [
    ['https://x.com/NASA/status/1234567890', 'twitter', 'video', '1234567890', false],
    ['https://twitter.com/i/web/status/99', 'twitter', 'video', '99', false],
    ['https://vimeo.com/76979871', 'vimeo', 'video', '76979871', false],
    ['https://player.vimeo.com/video/76979871', 'vimeo', 'video', '76979871', false],
    ['https://vimeo.com/channels/staffpicks/76979871', 'vimeo', 'video', '76979871', false],
    ['https://vimeo.com/showcase/1234567', 'vimeo', 'playlist', '1234567', true],
    ['https://vimeo.com/album/1234567', 'vimeo', 'playlist', '1234567', true],
    ['https://vimeo.com/channels/staffpicks', 'vimeo', 'playlist', 'staffpicks', true],
    ['https://www.twitch.tv/videos/2345678', 'twitch', 'video', '2345678', false],
    ['https://clips.twitch.tv/FunnySlug-abc', 'twitch', 'video', 'FunnySlug-abc', false],
    ['https://www.twitch.tv/chan/clip/Slug_1', 'twitch', 'video', 'Slug_1', false],
    ['https://www.twitch.tv/somechan/videos', 'twitch', 'playlist', 'somechan', true],
    ['https://artist.bandcamp.com/track/song-one', 'bandcamp', 'track', 'song-one', false],
    ['https://artist.bandcamp.com/album/record', 'bandcamp', 'album', 'record', true],
    ['https://artist.bandcamp.com/music', 'bandcamp', 'album', 'artist', true],
    ['https://www.reddit.com/r/videos/comments/abc123/title/', 'reddit', 'video', 'abc123', false],
    ['https://www.dailymotion.com/video/x2mvxf6', 'dailymotion', 'video', 'x2mvxf6', false],
    ['https://dai.ly/x2mvxf6', 'dailymotion', 'video', 'x2mvxf6', false],
    ['https://www.dailymotion.com/playlist/x6hz', 'dailymotion', 'playlist', 'x6hz', true],
    ['https://www.mixcloud.com/someuser/some-show/', 'mixcloud', 'track', 'some-show', false],
    ['https://www.mixcloud.com/someuser/', 'mixcloud', 'playlist', 'someuser', true],
    ['https://rumble.com/v5abcd-some-title.html', 'rumble', 'video', 'v5abcd-some-title.html', false],
  ];
  it.each(cases)('%s', (url, platform, type, id, isCollection) => {
    const d = detectUrl(url);
    expect(d.platform).toBe(platform);
    expect(d.contentType).toBe(type);
    expect(d.id).toBe(id);
    expect(d.isCollection).toBe(isCollection);
  });

  it('labels with the display name', () => {
    expect(detectUrl('https://x.com/a/status/1').label).toBe('X / Twitter video');
  });

  it.each([
    'https://twitch.tv.evil.com/videos/123',
    'https://evil.com/twitch.tv/videos/123',
    'https://notx.com/a/status/1',
    'https://x.com.evil.io/a/status/1',
    'https://vimeo.com.evil.io/76979871',
    'https://evilbandcamp.com/track/x',
    'https://bandcamp.com.evil.io/track/x',
    'https://evil.com/?u=https://vimeo.com/76979871',
    'https://fakemixcloud.com/u/show/',
  ])('lookalike %s stays generic', (url) => {
    expect(detectUrl(url).platform).toBe('generic');
  });

  it('a bare Twitch channel (live) is not treated as a collection', () => {
    const d = detectUrl('https://www.twitch.tv/somechan');
    expect(d.platform).toBe('generic');
    expect(d.isCollection).toBe(false);
  });
});
