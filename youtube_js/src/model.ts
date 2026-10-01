import {
  Album,
  Artist,
  Duration,
  InnerSong,
  Playlist,
  Song,
} from "wasm-extension-js";
import Innertube, { Misc, YT, YTMusic, YTNodes } from "youtubei.js";

type AsyncReturnType<T extends (...args: any) => any> = Awaited<ReturnType<T>>;
type MusicSearchResponse = AsyncReturnType<Innertube["music"]["search"]>;
type YtPlaylist = MusicSearchResponse["playlists"]["contents"][0];
type YtAlbum = MusicSearchResponse["albums"]["contents"][0];
type YtArtist = MusicSearchResponse["artists"]["contents"][0];
type Thumbnail = MusicSearchResponse["songs"]["contents"][0]["thumbnails"][0];

export function sanitizeId(id: string): string {
  if (!id) return "";
  let clean = id;
  if (clean.startsWith("moosync.youtube:")) {
    clean = clean.replace("moosync.youtube:", "");
  }
  const prefixes = [
    "youtube-playlist:",
    "youtube-artist:",
    "youtube-album:",
    "youtube:",
  ];
  for (const prefix of prefixes) {
    if (clean.startsWith(prefix)) {
      clean = clean.slice(prefix.length);
      break;
    }
  }
  return clean;
}

export function extractVideoId(input: string): string {
  const clean = sanitizeId(input);
  try {
    const url = new URL(clean);
    if (url.hostname.includes("youtu.be")) {
      return url.pathname.slice(1).split("/")[0].split("?")[0];
    }
    const v = url.searchParams.get("v");
    if (v) return v;
  } catch {
    // Not a URL, input is already an ID
  }
  return clean;
}

export function extractPlaylistId(input: string): string {
  const clean = sanitizeId(input);
  try {
    const url = new URL(clean);
    const list = url.searchParams.get("list");
    if (list) return list;
  } catch {
    // Not a URL, input is already an ID
  }
  return clean;
}

export function parseSong(
  input:
    | YTMusic.TrackInfo
    | YT.VideoInfo
    | YTNodes.MusicResponsiveListItem
    | YTNodes.MusicTwoRowItem
    | YTNodes.PlaylistVideo
    | undefined
    | null,
): Song | undefined {
  if (!input) return undefined;

  if (input instanceof YTMusic.TrackInfo || input instanceof YT.VideoInfo) {
    const info = input.basic_info;
    const id = info?.id;
    if (!id) return undefined;

    const artist = parseArtist(info);
    return new Song({
      artists: artist ? [artist] : [],
      song: new InnerSong({
        id: `youtube:${id}`,
        title: info.title,
        url: `https://www.youtube.com/watch?v=${id}`,
        duration: new Duration({
          seconds: BigInt(Math.floor(info.duration ?? 0)),
        }),
      }),
    });
  }

  if (input instanceof YTNodes.PlaylistVideo) {
    if (!input.id) return undefined;
    const artist = parseArtist(input.author);
    return new Song({
      artists: artist ? [artist] : [],
      song: new InnerSong({
        id: `youtube:${input.id}`,
        title: input.title?.text,
        url: `https://www.youtube.com/watch?v=${input.id}`,
        duration: new Duration({
          seconds: BigInt(Math.floor(input.duration?.seconds ?? 0)),
        }),
      }),
    });
  }

  if (input instanceof YTNodes.MusicResponsiveListItem) {
    if (!input.id) return undefined;
    const album = parseAlbum(input.album);
    return new Song({
      album,
      artists: input.artists?.map((a) => parseArtist(a)).filter(Boolean),
      song: new InnerSong({
        id: `youtube:${input.id}`,
        title: input.title ?? input.name,
        url:
          input.endpoint?.toURL() ?? `https://www.youtube.com/watch?v=${input.id}`,
        year: input.year,
        duration: new Duration({
          seconds: BigInt(Math.floor(input.duration?.seconds ?? 0)),
        }),
      }),
    });
  }

  if (input instanceof YTNodes.MusicTwoRowItem) {
    if (!input.id) return undefined;
    return new Song({
      artists: input.artists?.map((a) => parseArtist(a)).filter(Boolean),
      song: new InnerSong({
        id: `youtube:${input.id}`,
        title: input.title?.text,
        url:
          input.endpoint?.toURL() ?? `https://www.youtube.com/watch?v=${input.id}`,
        year: input.year,
      }),
    });
  }

  return undefined;
}

function getBestThumbnail(
  thumbnails?: Thumbnail[] | null,
): Thumbnail | undefined {
  return thumbnails?.reduce<Thumbnail | undefined>((best, current) => {
    if (!best) return current;
    return current.width > best.width ? current : best;
  }, undefined);
}

export function parsePlaylist(
  input:
    | YtPlaylist
    | YTNodes.Playlist
    | YTNodes.GridPlaylist
    | YTNodes.GridShow
    | YTNodes.LockupView
    | undefined
    | null,
): Playlist | undefined {
  if (!input) return undefined;

  if (input instanceof YTNodes.Playlist) {
    if (!input.id) return undefined;
    const bestThumb = getBestThumbnail(input.thumbnails);
    return new Playlist({
      playlistId: `youtube-playlist:${input.id}`,
      playlistName: input.title?.text,
      playlistCoverpath: bestThumb?.url,
      playlistSongCount: parseInt(input.video_count?.text ?? "0") || 0,
    });
  }

  if (input instanceof YTNodes.GridPlaylist) {
    if (!input.id) return undefined;
    const bestThumb = getBestThumbnail(input.thumbnails);
    return new Playlist({
      playlistId: `youtube-playlist:${input.id}`,
      playlistName: input.title?.text,
      playlistCoverpath: bestThumb?.url,
      playlistSongCount: parseInt(input.video_count?.text ?? "0") || 0,
    });
  }

  if (input instanceof YTNodes.GridShow) {
    return new Playlist({
      playlistName: input.title?.text,
    });
  }

  if (input instanceof YTNodes.LockupView) {
    if (!input.content_id) return undefined;
    return new Playlist({
      playlistId: `youtube-playlist:${input.content_id}`,
      playlistName: input.metadata?.title?.text,
    });
  }

  if (input.item_type !== "playlist") return undefined;
  if (!input.id) return undefined;

  return new Playlist({
    playlistId: `youtube-playlist:${input.id}`,
    playlistName: input.title,
    playlistCoverpath: getBestThumbnail(input.thumbnail?.contents)?.url,
    playlistSongCount: parseInt(input.song_count ?? "0") || 0,
  });
}

type SongMetadata = {
  id?: string;
  name?: string;
  title?: string;
  author?: string;
  channel?: { id?: string; name?: string };
  channel_id?: string;
  thumbnails?: Thumbnail[];
};

export function parseArtist(
  input?:
    | YTNodes.MusicResponsiveListItem
    | Misc.Author
    | SongMetadata
    | null,
): Artist | undefined {
  if (!input) return undefined;

  if (input instanceof Misc.Author) {
    if (!input.name) return undefined;
    return new Artist({
      artistId: input.id ? `youtube-artist:${input.id}` : undefined,
      artistName: input.name,
      artistCoverpath: input.best_thumbnail?.url,
    });
  }

  if (input instanceof YTNodes.MusicResponsiveListItem) {
    if (input.item_type !== "artist" || !input.id) return undefined;
    return new Artist({
      artistId: `youtube-artist:${input.id}`,
      artistName: input.name,
      artistCoverpath: getBestThumbnail(input.thumbnail?.contents)?.url,
    });
  }

  const name = input.author ?? input.channel?.name ?? input.name ?? input.title;
  if (!name) return undefined;

  const id = input.channel_id ?? input.channel?.id ?? input.id;
  const thumb = input.thumbnails;

  return new Artist({
    artistId: id ? `youtube-artist:${id}` : undefined,
    artistName: name,
    artistCoverpath: getBestThumbnail(thumb)?.url,
  });
}

export function parseAlbum(
  input?:
    | YTNodes.MusicResponsiveListItem
    | SongMetadata
    | null,
): Album | undefined {
  if (!input) return undefined;

  if (input instanceof YTNodes.MusicResponsiveListItem) {
    if (input.item_type !== "album" || !input.id) return undefined;
    return new Album({
      albumId: `youtube-album:${input.id}`,
      albumName: input.album?.name ?? input.title ?? input.name,
      albumArtist: input.artists?.[0]?.name ?? input.author?.name,
      albumCoverpathHigh: getBestThumbnail(input.thumbnail?.contents)?.url,
      albumCoverpathLow: getBestThumbnail(input.thumbnail?.contents)?.url,
      albumSongCount: parseInt(input.song_count ?? input.item_count ?? "0") || 0,
    });
  }

  const name = input.name ?? input.title;
  if (!name) return undefined;

  return new Album({
    albumId: input.id ? `youtube-album:${input.id}` : undefined,
    albumName: name,
    albumCoverpathHigh: getBestThumbnail(input.thumbnails)?.url,
    albumCoverpathLow: getBestThumbnail(input.thumbnails)?.url,
  });
}
