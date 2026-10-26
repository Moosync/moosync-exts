import {
  Duration,
  ExtensionAccountDetail,
  ExtensionProviderScope,
  GetProviderScopesResponse,
  OauthCallbackResponse,
  PerformAccountLoginResponse,
  PlaybackDetailsRequestedResponse,
  PreferenceData,
  RequestedAlbumSongsResponse,
  RequestedArtistSongsResponse,
  RequestedPlaylistFromUrlResponse,
  RequestedPlaylistSongsResponse,
  RequestedPlaylistsResponse,
  RequestedRecommendationsResponse,
  RequestedSearchResultResponse,
  RequestedSongFromUrlResponse,
  Song,
  getApi,
} from "wasm-extension-js";
import {
  extractPlaylistId,
  extractVideoId,
  parseAlbum,
  parseArtist,
  parsePlaylist,
  parseSong,
  sanitizeId,
} from "./model";

export { handle_extension_command } from "wasm-extension-js";
import { Innertube, OAuth2Tokens, Platform, Types, YTNodes } from "youtubei.js";

type Format = Awaited<ReturnType<Innertube["getStreamingData"]>>;

let extension: YoutubeJsExtension;
const api = getApi();
const CACHE_TTL_SECONDS = 7200;

class ExtensionCache implements Types.ICache {
  public readonly cache_dir: string = "";
  private mem = new Map<string, ArrayBuffer>();

  get(key: string): Promise<ArrayBuffer | undefined> {
    const memVal = this.mem.get(key);
    if (memVal) return Promise.resolve(memVal);

    const data = api.getSecure(new PreferenceData({ key }));
    const val = data?.value;
    if (val?.kind.case === "stringValue" && val.kind.value) {
      const buf = Uint8Array.from(atob(val.kind.value), (c) =>
        c.codePointAt(0) ?? 0,
      ).buffer;
      this.mem.set(key, buf);
      return Promise.resolve(buf);
    }
    return Promise.resolve(undefined);
  }

  set(key: string, value: ArrayBuffer): Promise<void> {
    this.mem.set(key, value);
    const bytes = new Uint8Array(value);
    let binary = "";
    for (let i = 0; i < bytes.byteLength; i++) {
      binary += String.fromCodePoint(bytes[i]);
    }
    const b64 = btoa(binary);
    api.setSecure(
      new PreferenceData({
        key,
        value: { kind: { case: "stringValue", value: b64 } },
      }),
    );
    return Promise.resolve();
  }

  remove(key: string): Promise<void> {
    this.mem.delete(key);
    api.setSecure(new PreferenceData({ key }));
    return Promise.resolve();
  }
}

interface PlaybackStreamCandidate {
  hlsManifestUrl?: string;
  formats?: Array<{ mimeType?: string; audioQuality?: string; url?: string }>;
  adaptiveFormats?: Array<{ mimeType?: string; url?: string }>;
}

function extractMusicPlaylistSongs(items: unknown[]): Song[] {
  const songs: Song[] = [];
  for (const item of items) {
    if (item instanceof YTNodes.MusicResponsiveListItem) {
      const parsed = parseSong(item);
      if (parsed) songs.push(parsed);
    }
  }
  return songs;
}

function extractYtPlaylistSongs(items: unknown[]): Song[] {
  const songs: Song[] = [];
  for (const item of items) {
    if (item instanceof YTNodes.PlaylistVideo) {
      const parsed = parseSong(item);
      if (parsed) songs.push(parsed);
    }
  }
  return songs;
}

function extractShelfSongs(contents: unknown[]): Song[] {
  const songs: Song[] = [];
  for (const item of contents) {
    if (
      item instanceof YTNodes.MusicResponsiveListItem ||
      item instanceof YTNodes.MusicTwoRowItem
    ) {
      const parsed = parseSong(item);
      if (parsed) songs.push(parsed);
    }
  }
  return songs;
}

function extractArtistSectionSongs(sections: unknown[]): Song[] {
  const songs: Song[] = [];
  for (const sec of sections) {
    if (
      sec instanceof YTNodes.MusicShelf ||
      sec instanceof YTNodes.MusicCarouselShelf
    ) {
      songs.push(...extractShelfSongs(sec.contents));
    }
  }
  return songs;
}

function extractStreamUrl(
  streamingData: PlaybackStreamCandidate | undefined,
  client: "IOS" | "ANDROID" | "WEB",
): string | undefined {
  if (streamingData?.hlsManifestUrl) {
    return streamingData.hlsManifestUrl;
  }

  const progressiveAudio = streamingData?.formats?.find(
    (f) =>
      (f.mimeType?.startsWith("audio/") ||
        (f.mimeType?.startsWith("video/") && f.audioQuality)) &&
      f.url,
  );
  if (progressiveAudio?.url) {
    return progressiveAudio.url;
  }

  if (client !== "IOS") {
    const adaptiveAudio = streamingData?.adaptiveFormats?.find(
      (f) => f.mimeType?.startsWith("audio/") && f.url,
    );
    if (adaptiveAudio?.url) {
      return adaptiveAudio.url;
    }
  }

  return undefined;
}

class YoutubeJsExtension {
  private innertube?: Innertube;
  private readonly cache = new ExtensionCache();

  public async init() {
    this.register_api();
    await this.sync_account();
    if ((globalThis as unknown as { Intl: unknown }).Intl === undefined) {
      (globalThis as unknown as { Intl: unknown }).Intl = {
        DateTimeFormat: () => ({
          resolvedOptions: () => ({ timeZone: "UTC" }),
        }),
      };
    }

    Platform.load({
      ...Platform.shim,
      eval: (data, _env) => {
        // NOSONAR: YouTube.js requires dynamic function evaluation for player deciphering
        const fn = new Function(data.output);
        const res = fn();
        if (typeof res === "object" && res !== null) {
          return res;
        }
        return undefined;
      },
    });

    try {
      this.innertube = await Innertube.create({
        timezone: "UTC",
        generate_session_locally: true,
      });

      this.innertube.session.on("auth-pending", (data) => {
        if (data.verification_url) {
          api.openExternalUrl(data.verification_url);
        }
      });

      this.innertube.session.on("auth", async () => {
        await this.sync_account();
      });

      this.innertube.session.on("update-credentials", async ({ credentials }) => {
        if (credentials) {
          await this.save_tokens(credentials);
        }
      });

      const tokens = await this.load_tokens();
      if (tokens) {
        try {
          await this.innertube.session.signIn(tokens);
          await this.sync_account();
        } catch (error_) {
          console.error("failed to signIn with stored tokens", error_);
        }
      }
    } catch (e) {
      console.error("failed to create innertube", e);
    }
  }
  

  private async save_tokens(tokens: OAuth2Tokens): Promise<void> {
    const json = JSON.stringify(tokens);
    const buf =  new TextEncoder().encode(json);

    let newBuf: ArrayBuffer;
    if (buf.buffer instanceof ArrayBuffer) {
      newBuf = buf.buffer;
    } else {
      newBuf = cloneToArrayBuffer(buf.buffer);
    }
    await this.cache.set("youtube_oauth_tokens", newBuf);
  }

  private async load_tokens(): Promise<OAuth2Tokens | undefined> {
    const buf = await this.cache.get("youtube_oauth_tokens");
    if (!buf) return undefined;
    const json = new TextDecoder().decode(buf);
    try {
      return JSON.parse(json);
    } catch {
      return undefined;
    }
  }

  private async sync_account(): Promise<void> {
    const loggedIn = this.innertube?.session.logged_in ?? false;
    let username: string | undefined;

    if (loggedIn && this.innertube) {
      try {
        const info = await this.innertube.account.getInfo();
        const items = info.contents?.contents;
        const accountItem =
          items?.find(
            (i): i is YTNodes.AccountItem =>
              i.is(YTNodes.AccountItem) && i.is_selected,
          ) ??
          items?.find((i): i is YTNodes.AccountItem =>
            i.is(YTNodes.AccountItem),
          );
        username =
          accountItem?.account_name?.toString() ??
          accountItem?.channel_handle?.toString();
      } catch (e) {
        console.error("failed to fetch account info", e);
      }
    }

    api.setAccount(
      new ExtensionAccountDetail({
        id: "youtube",
        name: "YouTube",
        packageName: "moosync.youtube_js",
        bgColor: "#FF0000",
        icon: "",
        loggedIn,
        username,
      }),
    );
  }

  private async fetchSongFromUrl(videoId: string): Promise<Song | undefined> {
    if (!this.innertube) return undefined;
    try {
      const trackInfo = await this.innertube.music.getInfo(videoId);
      return parseSong(trackInfo);
    } catch (error_) {
      console.error("music.getInfo failed", error_);
    }

    try {
      const videoInfo = await this.innertube.getInfo(videoId);
      return parseSong(videoInfo);
    } catch (error_) {
      console.error("getInfo failed", error_);
    }

    return undefined;
  }

  private async fetchPlaylistFromUrl(playlistId: string): Promise<Song[]> {
    if (!this.innertube) return [];
    try {
      const musicPlaylist = await this.innertube.music.getPlaylist(playlistId);
      return extractMusicPlaylistSongs(musicPlaylist.items);
    } catch {
      try {
        const ytPlaylist = await this.innertube.getPlaylist(playlistId);
        return extractYtPlaylistSongs(ytPlaylist.items);
      } catch {
        return [];
      }
    }
  }

  private async fetchArtistSongs(artistId: string): Promise<Song[]> {
    if (!this.innertube) return [];
    const ytArtist = await this.innertube.music.getArtist(artistId);
    try {
      const songsShelf = await ytArtist.getAllSongs();
      if (songsShelf) {
        return extractShelfSongs(songsShelf.contents);
      }
    } catch {
      // Fall through to sections
    }
    return extractArtistSectionSongs(ytArtist.sections);
  }

  private async getHomeFeedRecommendations(): Promise<Song[]> {
    if (!this.innertube) return [];
    const home = await this.innertube.music.getHomeFeed();
    const songs: Song[] = [];
    for (const sec of home.sections ?? []) {
      if (sec instanceof YTNodes.MusicCarouselShelf) {
        songs.push(...extractShelfSongs(sec.contents));
      }
    }
    return songs;
  }

  private async getExploreRecommendations(): Promise<Song[]> {
    if (!this.innertube) return [];
    const explore = await this.innertube.music.getExplore();
    const songs: Song[] = [];
    for (const sec of explore.sections) {
      songs.push(...extractShelfSongs(sec.contents));
    }
    return songs;
  }

  private async fetchStreamDetails(
    videoId: string,
    fallbackDuration: number,
  ): Promise<{ streamUrl?: string; durationSeconds: number }> {
    if (!this.innertube) {
      return { durationSeconds: fallbackDuration };
    }
    const clients: Array<"IOS" | "ANDROID" | "WEB"> = ["IOS", "ANDROID", "WEB"];
    let durationSeconds = fallbackDuration;

    for (const client of clients) {
      try {
        const response = await this.innertube.actions.execute("/player", {
          videoId,
          client,
        });
        const streamingData = response.data?.streamingData;
        const streamUrl = extractStreamUrl(streamingData, client);
        if (streamUrl) {
          if (response.data?.videoDetails?.lengthSeconds) {
            durationSeconds = Number(response.data.videoDetails.lengthSeconds);
          }
          return { streamUrl, durationSeconds };
        }
      } catch (err) {
        console.error(`Failed to fetch player data with client ${client}`, err);
      }
    }
    return { durationSeconds };
  }

  private currentDeviceCode?: {
    device_code: string;
    user_code: string;
    verification_url: string;
    interval: number;
  };

  private register_api() {
    api.on(
      "getProviderScopes",
      (_req) =>
        new GetProviderScopesResponse({
          scopes: [
            ExtensionProviderScope.SEARCH,
            ExtensionProviderScope.SONG_FROM_URL,
            ExtensionProviderScope.PLAYLIST_FROM_URL,
            ExtensionProviderScope.PLAYLISTS,
            ExtensionProviderScope.PLAYLIST_SONGS,
            ExtensionProviderScope.ARTIST_SONGS,
            ExtensionProviderScope.ALBUM_SONGS,
            ExtensionProviderScope.RECOMMENDATIONS,
            ExtensionProviderScope.ACCOUNTS,
            ExtensionProviderScope.PLAYBACK_DETAILS
          ],
        }),
    );

    api.on("performAccountLogin", async ({ loginStatus }) => {
      if (!this.innertube) {
        return new PerformAccountLoginResponse();
      }

      if (loginStatus) {
        try {
          this.innertube.session.oauth.client_id ??=
            await this.innertube.session.oauth.getClientID();
          const deviceCode =
            await this.innertube.session.oauth.getDeviceAndUserCode();
          this.currentDeviceCode = deviceCode;
          if (deviceCode.verification_url) {
            api.openExternalUrl(deviceCode.verification_url);
          }
          return new PerformAccountLoginResponse({
            status: deviceCode.user_code,
          });
        } catch (e) {
          console.error("failed to start oauth flow", e);
          return new PerformAccountLoginResponse();
        }
      }

      this.currentDeviceCode = undefined;
      try {
        await this.innertube.session.signOut();
        await this.cache.remove("youtube_oauth_tokens");
      } catch (e) {
        console.error("failed to signOut", e);
      }
      await this.sync_account();
      return new PerformAccountLoginResponse();
    });

    api.on("oauthCallback", async ({ callbackUri }) => {
      if (!this.innertube) {
        return new OauthCallbackResponse();
      }

      const client = this.innertube.session.oauth.client_id ?? await this.innertube.session.oauth.getClientID();
      const deviceCode = this.currentDeviceCode?.device_code ?? callbackUri;

      if (client && deviceCode) {
        try {
          const res = await this.innertube.session.http.fetch_function(
            this.innertube.session.oauth.AUTH_SERVER_TOKEN_URL,
            {
              method: "POST",
              headers: { "Content-Type": "application/json" },
              body: JSON.stringify({
                client_id: client.client_id,
                client_secret: client.client_secret,
                code: deviceCode,
                grant_type: "http://oauth.net/grant_type/device/1.0", // NOSONAR
              }),
            },
          );
          const text = await res.text();
          const data: {
            access_token?: string;
            refresh_token?: string;
            expiry_date?: string;
            expires_in?: number;
            token_type?: string;
            scope?: string;
            error?: string;
          } = JSON.parse(text);

          if (data.access_token && data.refresh_token) {
            const tokens: OAuth2Tokens = {
              access_token: data.access_token,
              refresh_token: data.refresh_token,
              expiry_date:
                data.expiry_date ??
                new Date(
                  Date.now() + (data.expires_in ?? 3600) * 1000,
                ).toISOString(),
              token_type: data.token_type,
              scope: data.scope,
              client,
            };
            await this.innertube.session.signIn(tokens);
            await this.save_tokens(tokens);
          }
        } catch (e) {
          console.error("failed to exchange token in oauthCallback", e);
        }
      }

      await this.sync_account();
      return new OauthCallbackResponse();
    });

    api.on("requestedSongFromUrl", async ({ url }) => {
      if (!this.innertube || !url) {
        return new RequestedSongFromUrlResponse();
      }
      const videoId = extractVideoId(url);
      const cacheKey = `song_url:${videoId}`;
      const cached = api.getCache(cacheKey, RequestedSongFromUrlResponse);
      if (cached) {
        return cached;
      }
      const song = await this.fetchSongFromUrl(videoId);
      const resp = new RequestedSongFromUrlResponse({ song });
      if (song) {
        api.setCache(cacheKey, resp, CACHE_TTL_SECONDS);
      }
      return resp;
    });

    api.on("requestedPlaylistFromUrl", async ({ url }) => {
      if (!this.innertube || !url) {
        return new RequestedPlaylistFromUrlResponse();
      }
      const playlistId = extractPlaylistId(url);
      const cacheKey = `playlist_url:${playlistId}`;
      const cached = api.getCache(cacheKey, RequestedPlaylistFromUrlResponse);
      if (cached) {
        return cached;
      }
      const songs = await this.fetchPlaylistFromUrl(playlistId);
      const resp = new RequestedPlaylistFromUrlResponse({ songs });
      if (songs.length > 0) {
        api.setCache(cacheKey, resp, CACHE_TTL_SECONDS);
      }
      return resp;
    });

    api.on("requestedPlaylistSongs", async ({ id }) => {
      if (!this.innertube || !id) {
        return new RequestedPlaylistSongsResponse();
      }
      const playlistId = sanitizeId(id);
      const cacheKey = `playlist_songs:${playlistId}`;
      const cached = api.getCache(cacheKey, RequestedPlaylistSongsResponse);
      if (cached) {
        return cached;
      }
      try {
        const playlist = await this.innertube.music.getPlaylist(playlistId);
        const songs = extractMusicPlaylistSongs(playlist.items);
        const resp = new RequestedPlaylistSongsResponse({
          songs,
        });
        if (songs.length > 0) {
          api.setCache(cacheKey, resp, CACHE_TTL_SECONDS);
        }
        return resp;
      } catch (error_) {
        console.error("failed to get playlist songs", error_);
        return new RequestedPlaylistSongsResponse();
      }
    });

    api.on("requestedArtistSongs", async ({ artist }) => {
      if (!this.innertube || !artist?.artistId) {
        return new RequestedArtistSongsResponse();
      }
      const artistId = sanitizeId(artist.artistId);
      const cacheKey = `artist_songs:${artistId}`;
      const cached = api.getCache(cacheKey, RequestedArtistSongsResponse);
      if (cached) {
        return cached;
      }
      try {
        const songs = await this.fetchArtistSongs(artistId);
        const resp = new RequestedArtistSongsResponse({
          songs,
        });
        if (songs.length > 0) {
          api.setCache(cacheKey, resp, CACHE_TTL_SECONDS);
        }
        return resp;
      } catch (error_) {
        console.error("failed to get artist songs", error_);
        return new RequestedArtistSongsResponse();
      }
    });

    api.on("requestedAlbumSongs", async ({ album }) => {
      if (!this.innertube || !album?.albumId) {
        return new RequestedAlbumSongsResponse();
      }
      const albumId = sanitizeId(album.albumId);
      const cacheKey = `album_songs:${albumId}`;
      const cached = api.getCache(cacheKey, RequestedAlbumSongsResponse);
      if (cached) {
        return cached;
      }
      try {
        const ytAlbum = await this.innertube.music.getAlbum(albumId);
        const songs = ytAlbum.contents.map(parseSong).filter(Boolean);
        const resp = new RequestedAlbumSongsResponse({ songs });
        if (songs.length > 0) {
          api.setCache(cacheKey, resp, CACHE_TTL_SECONDS);
        }
        return resp;
      } catch (error_) {
        console.error("failed to get album songs", error_);
        return new RequestedAlbumSongsResponse();
      }
    });

    api.on("requestedRecommendations", async () => {
      if (!this.innertube) {
        return new RequestedRecommendationsResponse();
      }
      const loggedIn = this.innertube.session.logged_in;
      const cacheKey = `recommendations:${loggedIn ? "auth" : "anon"}`;
      const cached = api.getCache(cacheKey, RequestedRecommendationsResponse);
      if (cached) {
        return cached;
      }
      try {
        const songs = loggedIn
          ? await this.getHomeFeedRecommendations()
          : await this.getExploreRecommendations();
        const resp = new RequestedRecommendationsResponse({ songs });
        if (songs.length > 0) {
          api.setCache(cacheKey, resp, CACHE_TTL_SECONDS);
        }
        return resp;
      } catch (error_) {
        console.error("failed to get recommendations", error_);
        return new RequestedRecommendationsResponse();
      }
    });

    api.on("requestedPlaylists", async () => {
      if (!this.innertube?.session.logged_in) {
        return new RequestedPlaylistsResponse();
      }
      const cacheKey = "user_playlists";
      const cached = api.getCache(cacheKey, RequestedPlaylistsResponse);
      if (cached) {
        return cached;
      }
      try {
        const feed = await this.innertube.getPlaylists();
        const playlists = feed.playlists.map(parsePlaylist).filter(Boolean);
        const resp = new RequestedPlaylistsResponse({ playlists });
        if (playlists.length > 0) {
          api.setCache(cacheKey, resp, CACHE_TTL_SECONDS);
        }
        return resp;
      } catch (error_) {
        console.error("failed to get playlists", error_);
        return new RequestedPlaylistsResponse();
      }
    });

    api.on("requestedSearchResult", async ({ query }) => {
      if (!this.innertube) {
        return new RequestedSearchResultResponse();
      }
      const cacheKey = `search:${query.trim()}`;
      const cached = api.getCache(cacheKey, RequestedSearchResultResponse);
      if (cached) {
        return cached;
      }
      const songSearchRes = await this.innertube.music.search(query, {
        type: "song",
      });
      const playlistSearchRes = await this.innertube.music.search(query, {
        type: "playlist",
      });
      const artistSearchRes = await this.innertube.music.search(query, {
        type: "artist",
      });
      const albumSearchRes = await this.innertube.music.search(query, {
        type: "album",
      });

      if ((songSearchRes.songs?.contents.length ?? 0) < 10) {
        const searchRes2 = await songSearchRes.getMore(songSearchRes.songs);
        songSearchRes.albums?.contents.push(...(searchRes2.albums?.contents ?? []));
        songSearchRes.songs?.contents.push(...(searchRes2.songs?.contents ?? []));
        songSearchRes.artists?.contents.push(...(searchRes2.artists?.contents ?? []));
        songSearchRes.playlists?.contents.push(
          ...(searchRes2.playlists?.contents ?? []),
        );
      }

      const resp = new RequestedSearchResultResponse({
        songs: songSearchRes.songs?.contents.map(parseSong).filter(Boolean),
        playlists: playlistSearchRes.playlists?.contents
          .map(parsePlaylist)
          .filter(Boolean),
        artists: artistSearchRes.artists?.contents
          .map(parseArtist)
          .filter(Boolean),
        albums: albumSearchRes.albums?.contents.map(parseAlbum).filter(Boolean),
      });
      api.setCache(cacheKey, resp, CACHE_TTL_SECONDS);
      return resp;
    });

    api.on("playbackDetailsRequested", async ({ song }) => {
      if (!this.innertube || !song) {
        return new PlaybackDetailsRequestedResponse();
      }

      const rawId =
        song.song?.id ?? song.song?.playbackUrl ?? song.song?.url ?? "";
      const videoId = extractVideoId(rawId);
      if (!videoId) {
        return new PlaybackDetailsRequestedResponse();
      }

      const fallbackDuration = song.song?.duration?.seconds
        ? Number(song.song.duration.seconds)
        : 0;

      const { streamUrl, durationSeconds } = await this.fetchStreamDetails(
        videoId,
        fallbackDuration,
      );

      if (!streamUrl) {
        console.error(`No playback stream available for video: ${videoId}`);
        return new PlaybackDetailsRequestedResponse();
      }

      return new PlaybackDetailsRequestedResponse({
        url: streamUrl,
        duration:
          durationSeconds > 0
            ? new Duration({
                seconds: BigInt(Math.floor(durationSeconds)),
              })
            : undefined,
      });
    });
  }
}

export function entry(): number {
  extension = new YoutubeJsExtension();
  void extension.init();
  console.log("started youtubejs extension successfully");
  return 0;
}

function cloneToArrayBuffer(bufferLike: SharedArrayBuffer): ArrayBuffer {
  const byteLength = bufferLike.byteLength;
  
  const targetBuffer = new ArrayBuffer(byteLength);
  
  const sourceView = new Uint8Array(bufferLike);
  const targetView = new Uint8Array(targetBuffer); 
  
  targetView.set(sourceView);
  
  return targetBuffer;
}
