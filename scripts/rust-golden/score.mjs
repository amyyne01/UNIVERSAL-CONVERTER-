// Golden file for downloader/score.rs: scoreCandidates over a matrix of Spotify
// tracks and candidate lists. Durations and scores also carry their IEEE bits,
// so the comparison is exact whatever the JSON number parser does.
import { writeFileSync } from 'node:fs';

export default async function ({ OUT }) {
  const { scoreCandidates } = await import('../../dist-electron/electron/downloader.js');
  const f64 = new Float64Array(1);
  const u64 = new BigUint64Array(f64.buffer);
  const bits = (x) => { f64[0] = x; return u64[0].toString(16); };

  const mk = (artist, name, durationMs) => ({
    id: `t-${artist}-${name}`, name, artist, artists: artist.split(', '), album: 'Album', trackNumber: 1, durationMs,
    thumbnailUrl: '',
  });
  const tracks = [
    mk('Queen', 'Bohemian Rhapsody', 355000),
    mk('Daft Punk, Pharrell Williams', 'Get Lucky (feat. Pharrell Williams)', 369000),
    mk('Beyoncé', 'Halo', 261000),
    mk('Sigur Rós', 'Hoppípolla', 268000),
    mk('AC/DC', 'T.N.T.', 215000),
    mk("Guns N' Roses", "Sweet Child O' Mine", 356000),
    mk('Queen', 'Bohemian Rhapsody', 0),
    mk('Queen', 'Bohemian Rhapsody', -5000),
    mk('Queen', 'Bohemian Rhapsody', 1),
    mk('Queen', 'Bohemian Rhapsody', 200500),
    mk('Queen', 'Bohemian Rhapsody', 12000),
    mk('Queen', 'Bohemian Rhapsody', 123457),
    mk('Wings', 'Live and Let Die', 192000),
    mk('Marshmello', 'Alone (Remix)', 200000),
    mk('Some Band', 'Cover Me', 180000),
    mk('Deadmau5', '8D Dream', 210000),
    mk('', 'Orphan Song', 150000),
    mk('Nobody', '', 150000),
    mk('', '', 150000),
    mk('ΣΙΣΥΦΟΣ', 'İstanbul Night', 240000),
    mk('Björk', '日本語 Mix', 240000),
    mk('The The', 'The The', 100000),
    mk('Sped Up', 'Nightcore Anthem', 100000),
    mk('Artist', 'Song', 100000.5),
  ];

  const src = (id, title, uploader, duration) => ({ id, title, uploader, duration, thumbnailUrl: '', url: `https://youtu.be/${id}`, source: 'youtube' });
  const lists = (t) => {
    const sec = t.durationMs > 0 ? t.durationMs / 1000 : 0;
    const title = `${t.artist} - ${t.name}`;
    const base = sec || 200;
    const exact = src('exact', title, t.artist, sec);
    const topic = src('topic', t.name, `${t.artist} - Topic`, sec);
    const own = src('own', t.name, t.artist, sec);
    const plain = src('plain', title, 'Random Uploader', sec);
    const noDur = src('nodur', title, `${t.artist} - Topic`, 0);
    const kws = ['live', 'cover', 'remix', 'sped', '8d', 'nightcore'].map((k, i) => src(`kw-${k}`, `${title} ${i % 2 ? k.toUpperCase() : `(${k})`}`, t.artist, sec));
    const edge = [0, 5, 11.999, 12, 12.001, 13].flatMap((dx) => [
      src(`plus-${dx}`, title, t.artist, base + dx), src(`minus-${dx}`, title, t.artist, base - dx),
    ]);
    const odd = [
      src('neg', title, t.artist, -3), src('tiny', title, t.artist, 0.001), src('empty-up', title, '', sec),
      src('empty-title', '', t.artist, sec), src('unicode', `${title} é日`, `${t.artist} - TOPIC`, sec),
    ];
    const ties = [src('tie-a', title, 'x', sec), src('tie-b', title, 'x', sec), src('tie-c', title, 'x', sec), exact, own];
    const all = [...kws, ...edge, ...odd, ...ties, plain, exact, topic, own, noDur];
    return [
      [], [exact], [topic], [own], [noDur], kws, edge, odd, ties, [...ties].reverse(),
      [plain, exact, topic, own, noDur], all, [...all].reverse(),
    ];
  };

  const record = (track, candidates) => ({
    track, candidates, trackBits: bits(track.durationMs), candidateBits: candidates.map((c) => bits(c.duration)),
    out: scoreCandidates(track, candidates).map(({ candidate, score }) => ({ id: candidate.id, score, bits: bits(score) })),
  });
  const cases = tracks.flatMap((track) => lists(track).map((candidates) => record(track, candidates)));
  // Thirds and fifths, where the order of operations shows in the last digit.
  const thirds = mk('A B C', 'D E F G H I J K L M N', 100000);
  const big = Array.from({ length: 12 }, (_, i) => src(`c${i}`, ['a', 'b c', 'a d e', 'f g h i', 'x'][i % 5] + ` ${'z '.repeat(i % 3)}live`, i % 2 ? 'a b c - Topic' : 'a', 100 + (i % 4) * 2.9));
  cases.push(record(thirds, big));
  writeFileSync(`${OUT}/score.json`, JSON.stringify(cases, null, 1) + '\n');
  console.log(`score.json: ${cases.length} cases`);
}
