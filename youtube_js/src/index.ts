import {
  Duration,
  ExtensionAccountDetail,
  GetSecureRequest,
  OauthCallbackResponse,
  OpenExternalUrlRequest,
  PerformAccountLoginResponse,
  PlaybackDetailsRequestedResponse,
  PreferenceData,
  RegisterOauthRequest,
  RequestedAlbumSongsResponse,
  RequestedArtistSongsResponse,
  RequestedPlaylistSongsResponse,
  RequestedPlaylistsResponse,
  RequestedRecommendationsResponse,
  RequestedSearchResultResponse,
  SetAccountRequest,
  SetSecureRequest,
  Song,
  SongType,
  Timestamp,
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
import {
  albumSongsCacheKey,
  artistSongsCacheKey,
  playbackCacheKey,
  playlistSongsCacheKey,
  recommendationsCacheKey,
  searchCacheKey,
  userPlaylistsCacheKey,
} from "./cache_keys";

export { handle_extension_command } from "wasm-extension-js";
import { Innertube, OAuth2Tokens, Platform, YTNodes } from "youtubei.js";

type Format = Awaited<ReturnType<Innertube["getStreamingData"]>>;

let extension: YoutubeJsExtension;
const api = getApi();
const CACHE_TTL_SECONDS = 3600;

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
      await api.registerOauth(
        new RegisterOauthRequest({ url: "moosync.youtube.js" }),
      );
      const baseFetch = Platform.shim.fetch;
      const fetchWithAuthFallback: typeof Platform.shim.fetch = async (
        input,
        init,
      ) => {
        const isReq = input instanceof Request;
        const clonedBody =
          isReq && typeof input.clone === "function" && input.body !== null
            ? await input.clone().text()
            : undefined;
        const response = await baseFetch(input, init);
        if (response.status !== 400) {
          return response;
        }
        const headers = new Headers(
          init?.headers ?? (isReq ? input.headers : undefined),
        );
        if (!headers.has("authorization") && !headers.has("Authorization")) {
          return response;
        }
        headers.delete("authorization");
        headers.delete("Authorization");
        const retryInput = isReq ? input.url : input;
        const retryInit: RequestInit = isReq
          ? {
              method: init?.method ?? input.method,
              headers,
              body: init?.body ?? clonedBody,
            }
          : { ...init, headers };
        return baseFetch(retryInput, retryInit);
      };
      this.innertube = await Innertube.create({
        timezone: "UTC",
        generate_session_locally: true,
        fetch: fetchWithAuthFallback,
      });

      this.innertube.session.on("auth-pending", async (data) => {
        if (data.verification_url) {
          await api.openExternalUrl(
            new OpenExternalUrlRequest({ url: data.verification_url }),
          );
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
    await api.setSecure(
      new SetSecureRequest({
        data: new PreferenceData({
          key: "youtube_oauth_tokens",
          value: { kind: { case: "stringValue", value: json } },
        }),
      }),
    );
  }

  private async load_tokens(): Promise<OAuth2Tokens | undefined> {
    const res = await api.getSecure(
      new GetSecureRequest({
        data: new PreferenceData({ key: "youtube_oauth_tokens" }),
      }),
    );
    const text =
      res?.data?.value?.kind.case === "stringValue"
        ? res.data.value.kind.value
        : undefined;
    if (text) {
      try {
        return JSON.parse(text);
      } catch (e) {
        console.error("failed to parse stored oauth tokens", e);
      }
    }
    return undefined;
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

    await api.setAccount(
      new SetAccountRequest({
        account: new ExtensionAccountDetail({
          id: "youtube",
          name: "YouTube",
          packageName: "moosync.youtube_js",
          bgColor: "#FF0000",
          icon: "",
          loggedIn,
          username,
        }),
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
  ): Promise<{
    streamUrl?: string;
    durationSeconds: number;
    expiresInSeconds?: number;
  }> {
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
          const rawExpires = streamingData?.expiresInSeconds;
          const expiresInSeconds = rawExpires
            ? parseInt(rawExpires, 10)
            : undefined;
          return { streamUrl, durationSeconds, expiresInSeconds };
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
            await api.openExternalUrl(
              new OpenExternalUrlRequest({ url: deviceCode.verification_url }),
            );
          }
          return new PerformAccountLoginResponse({
            url: deviceCode.verification_url ?? "",
            deviceCode: deviceCode.user_code,
          });
        } catch (e) {
          console.error("failed to start oauth flow", e);
          return new PerformAccountLoginResponse();
        }
      }

      this.currentDeviceCode = undefined;
      try {
        await this.innertube.session.signOut();
        await api.setSecure(
          new SetSecureRequest({
            data: new PreferenceData({ key: "youtube_oauth_tokens" }),
          }),
        );
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

    api.on("requestedPlaylistSongs", async ({ id }) => {
      if (!this.innertube || !id) {
        return new RequestedPlaylistSongsResponse();
      }
      const playlistId = sanitizeId(id);
      const cacheKey = playlistSongsCacheKey(playlistId);
      const cached = await api.getCache(cacheKey, RequestedPlaylistSongsResponse);
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
          await api.setCache(cacheKey, resp, CACHE_TTL_SECONDS);
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
      const cacheKey = artistSongsCacheKey(artistId);
      const cached = await api.getCache(cacheKey, RequestedArtistSongsResponse);
      if (cached) {
        return cached;
      }
      try {
        const songs = await this.fetchArtistSongs(artistId);
        const resp = new RequestedArtistSongsResponse({
          songs,
        });
        if (songs.length > 0) {
          await api.setCache(cacheKey, resp, CACHE_TTL_SECONDS);
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
      const cacheKey = albumSongsCacheKey(albumId);
      const cached = await api.getCache(cacheKey, RequestedAlbumSongsResponse);
      if (cached) {
        return cached;
      }
      try {
        const ytAlbum = await this.innertube.music.getAlbum(albumId);
        const songs = ytAlbum.contents.map(parseSong).filter(Boolean);
        const resp = new RequestedAlbumSongsResponse({ songs });
        if (songs.length > 0) {
          await api.setCache(cacheKey, resp, CACHE_TTL_SECONDS);
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
      const cacheKey = recommendationsCacheKey(loggedIn ? "auth" : "anon");
      const cached = await api.getCache(cacheKey, RequestedRecommendationsResponse);
      if (cached) {
        return cached;
      }
      try {
        const songs = loggedIn
          ? await this.getHomeFeedRecommendations()
          : await this.getExploreRecommendations();
        const resp = new RequestedRecommendationsResponse({ songs });
        if (songs.length > 0) {
          await api.setCache(cacheKey, resp, CACHE_TTL_SECONDS);
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
      const cacheKey = userPlaylistsCacheKey();
      const cached = await api.getCache(cacheKey, RequestedPlaylistsResponse);
      if (cached) {
        return cached;
      }
      try {
        const innertube = this.innertube;
        const playlists = await innertube
          .getPlaylists()
          .then((feed) => feed.playlists.map(parsePlaylist).filter(Boolean))
          .catch(async (primaryError) => {
            console.warn(
              "getPlaylists failed, falling back to music.getLibrary",
              primaryError,
            );
            const library = await innertube.music.getLibrary();
            const libraryItems =
              library.contents?.flatMap((section) => {
                if ("items" in section && Array.isArray(section.items)) {
                  return section.items;
                }
                if ("contents" in section && Array.isArray(section.contents)) {
                  return section.contents;
                }
                return [];
              }) ?? [];
            return libraryItems.map(parsePlaylist).filter(Boolean);
          });
        const resp = new RequestedPlaylistsResponse({ playlists });
        if (playlists.length > 0) {
          await api.setCache(cacheKey, resp, CACHE_TTL_SECONDS);
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
      const cacheKey = searchCacheKey(query);
      const cached = await api.getCache(cacheKey, RequestedSearchResultResponse);
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
      await api.setCache(cacheKey, resp, CACHE_TTL_SECONDS);
      return resp;
    });

    api.on("playbackDetailsRequested", async ({ song }) => {
      if (!this.innertube || !song) {
        return new PlaybackDetailsRequestedResponse();
      }

      const ext = song.song?.extension ?? "";
      const isYoutubeOrigin =
        !ext ||
        ext === "moosync.youtube.js" ||
        ext === "youtube_js" ||
        ext === "youtube";
      const songType = song.song?.type;
      const isYoutubeType = songType !== SongType.SPOTIFY;
      const rawId =
        song.song?.id ?? song.song?.playbackUrl?.url ?? song.song?.url ?? "";
      const cleanId = sanitizeId(rawId);
      const isYoutubeUrlOrStrictId =
        /^[a-zA-Z0-9_-]{11}$/.test(cleanId) ||
        cleanId.includes("youtube.com") ||
        cleanId.includes("youtu.be");

      let videoId =
        isYoutubeOrigin && isYoutubeType && isYoutubeUrlOrStrictId
          ? extractVideoId(rawId)
          : "";
      if (!videoId) {
        const title = song.song?.title ?? "";
        const artists = song.artists
          .map((a) => a.artistName ?? "")
          .filter(Boolean)
          .join(" ");
        const searchQuery = `${artists} ${title}`.trim();
        if (!searchQuery) {
          return new PlaybackDetailsRequestedResponse();
        }
        const searchRes = await this.innertube.music.search(searchQuery, {
          type: "song",
        });
        const firstSong = searchRes.songs?.contents
          .map(parseSong)
          .find((s) => Boolean(s?.song?.id));
        videoId = extractVideoId(firstSong?.song?.id ?? "");
      }
      if (!videoId) {
        return new PlaybackDetailsRequestedResponse();
      }

      const cacheKey = playbackCacheKey(videoId);
      const cached = await api.getCache(cacheKey, PlaybackDetailsRequestedResponse);
      if (cached) {
        return cached;
      }

      const fallbackDuration = song.song?.duration?.seconds
        ? Number(song.song.duration.seconds)
        : 0;

      const { streamUrl, durationSeconds, expiresInSeconds } =
        await this.fetchStreamDetails(videoId, fallbackDuration);

      if (!streamUrl) {
        console.error(`No playback stream available for video: ${videoId}`);
        return new PlaybackDetailsRequestedResponse();
      }

      const ttlSeconds =
        expiresInSeconds && expiresInSeconds > 0 ? expiresInSeconds : 20 * 60;
      const expiresAtSeconds = Math.floor(Date.now() / 1000) + ttlSeconds;

      const resp = new PlaybackDetailsRequestedResponse({
        url: streamUrl,
        duration:
          durationSeconds > 0
            ? new Duration({
                seconds: BigInt(Math.floor(durationSeconds)),
              })
            : undefined,
        expiresAt: new Timestamp({
          seconds: BigInt(expiresAtSeconds),
        }),
      });

      if (expiresInSeconds && expiresInSeconds > 60) {
        await api.setCache(cacheKey, resp, expiresInSeconds - 60);
      }

      return resp;
    });
  }
}

export function entry(): number {
  extension = new YoutubeJsExtension();
  extension.init();
  console.log("started youtubejs extension successfully");
  return 0;
}
