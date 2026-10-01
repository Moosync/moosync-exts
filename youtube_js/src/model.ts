import {
  Album,
  Artist,
  Duration,
  InnerSong,
  Playlist,
  Song,
} from "wasm-extension-js";
import Innertube from "youtubei.js";

type AsyncReturnType<T extends (...args: any) => any> = Awaited<ReturnType<T>>;
type MusicSearchResponse = AsyncReturnType<Innertube["music"]["search"]>;
type YtMusic = MusicSearchResponse["songs"]["contents"][0];
type YtPlaylist = MusicSearchResponse["playlists"]["contents"][0];
type YtAlbum = MusicSearchResponse["albums"]["contents"][0];
type YtArtist = MusicSearchResponse["artists"]["contents"][0];
type Thumbnail = YtMusic["thumbnails"][0];

export function parseSong(input: YtMusic): Song | undefined {
  if (input.item_type !== "song") return undefined;
  if (!input.id) return undefined;

  return new Song({
    album: input.album?.id
      ? new Album({
          albumId: `youtube-album:${input.album?.id}`,
          albumName: input.album?.name,
        })
      : undefined,
    artists: input.artists?.map(
      (artist) =>
        new Artist({
          artistId: `youtube-artist:${artist.channel_id}`,
          artistName: artist.name,
        }),
    ),
    song: new InnerSong({
      id: `youtube:${input.id}`,
      title: input.title,
      url: input.endpoint?.toURL(),
      year: input.year,
      duration: new Duration({
        seconds: BigInt(input.duration?.seconds),
      }),
    }),
  });
}

function getBestThumbnail(thumbnails: Thumbnail[]): Thumbnail {
  return thumbnails.reduce((best, current) => {
    if (!best) return current;
    return current.width > best.width ? current : best;
  }, undefined);
}

export function parsePlaylist(input: YtPlaylist): Playlist | undefined {
  if (input.item_type !== "playlist") return undefined;
  if (!input.id) return undefined;

  return new Playlist({
    playlistId: `youtube-playlist:${input.id}`,
    playlistName: input.title,
    playlistCoverpath: getBestThumbnail(input.thumbnail?.contents)?.url,
    playlistSongCount: input.song_count ? parseInt(input.song_count) : 0,
  });
}

export function parseArtist(input: YtArtist): Artist | undefined {
  if (input.item_type !== "artist") return undefined;
  if (!input.id) return undefined;

  return new Artist({
    artistId: `youtube-artist:${input.id}`,
    artistName: input.name,
    artistCoverpath: getBestThumbnail(input.thumbnail?.contents)?.url,
  });
}

export function parseAlbum(input: YtAlbum): Album | undefined {
  if (input.item_type !== "album") return undefined;
  if (!input.id) return undefined;

  return new Album({
    albumId: `youtube-album:${input.id}`,
    albumName: input.album?.name,
    albumArtist: input.artists?.[0]?.name,
    albumCoverpathHigh: getBestThumbnail(input.thumbnail?.contents)?.url,
    albumCoverpathLow: getBestThumbnail(input.thumbnail?.contents)?.url,
    albumSongCount: input.song_count ? parseInt(input.song_count) : 0,
  });
}
