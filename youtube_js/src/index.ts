import {
  ExtensionAccountDetail,
  ExtensionProviderScope,
  GetProviderScopesResponse,
  OauthCallbackResponse,
  PerformAccountLoginResponse,
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
import { Innertube, OAuth2Tokens, Types, YTNodes } from "youtubei.js";

let extension: YoutubeJsExtension;
const api = getApi();

class ExtensionCache implements Types.ICache {
  public cache_dir: string = "";
  private mem = new Map<string, ArrayBuffer>();

  async get(key: string): Promise<ArrayBuffer | undefined> {
    const memVal = this.mem.get(key);
    if (memVal) return memVal;

    const data = api.getSecure(new PreferenceData({ key }));
    const val = data?.value;
    if (val?.kind.case === "stringValue" && val.kind.value) {
      const buf = Uint8Array.from(atob(val.kind.value), (c) =>
        c.charCodeAt(0),
      ).buffer;
      this.mem.set(key, buf);
      return buf;
    }
    return undefined;
  }

  async set(key: string, value: ArrayBuffer): Promise<void> {
    this.mem.set(key, value);
    const bytes = new Uint8Array(value);
    let binary = "";
    for (let i = 0; i < bytes.byteLength; i++) {
      binary += String.fromCharCode(bytes[i]);
    }
    const b64 = btoa(binary);
    api.setSecure(
      new PreferenceData({
        key,
        value: { kind: { case: "stringValue", value: b64 } },
      }),
    );
  }

  async remove(key: string): Promise<void> {
    this.mem.delete(key);
    api.setSecure(new PreferenceData({ key }));
  }
}

class YoutubeJsExtension {
  private innertube?: Innertube;
  private cache = new ExtensionCache();

  public async init() {
    this.register_api();
    if (typeof (globalThis as unknown as { Intl: unknown }).Intl === "undefined") {
      (globalThis as unknown as { Intl: unknown }).Intl = {
        DateTimeFormat: () => ({
          resolvedOptions: () => ({ timeZone: "UTC" }),
        }),
      };
    }

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
          ],
        }),
    );

    api.on("performAccountLogin", async ({ loginStatus }) => {
      if (!this.innertube) {
        return new PerformAccountLoginResponse();
      }

      if (loginStatus) {
        try {
          if (!this.innertube.session.oauth.client_id) {
            this.innertube.session.oauth.client_id =
              await this.innertube.session.oauth.getClientID();
          }
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
                grant_type: "http://oauth.net/grant_type/device/1.0",
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
      try {
        const trackInfo = await this.innertube.music.getInfo(videoId);
        const song = parseSong(trackInfo);
        return new RequestedSongFromUrlResponse({ song });
      } catch (e1) {
        console.error("music.getInfo failed", e1);
        try {
          const videoInfo = await this.innertube.getInfo(videoId);
          const song = parseSong(videoInfo);
          return new RequestedSongFromUrlResponse({ song });
        } catch (e2) {
          console.error("getInfo failed", e2);
          return new RequestedSongFromUrlResponse();
        }
      }
    });

    api.on("requestedPlaylistFromUrl", async ({ url }) => {
      if (!this.innertube || !url) {
        return new RequestedPlaylistFromUrlResponse();
      }
      const playlistId = extractPlaylistId(url);
      try {
        const musicPlaylist = await this.innertube.music.getPlaylist(playlistId);
        const songs: Song[] = [];
        for (const item of musicPlaylist.items) {
          if (item instanceof YTNodes.MusicResponsiveListItem) {
            const parsed = parseSong(item);
            if (parsed) songs.push(parsed);
          }
        }
        return new RequestedPlaylistFromUrlResponse({ songs });
      } catch {
        try {
          const ytPlaylist = await this.innertube.getPlaylist(playlistId);
          const songs: Song[] = [];
          for (const item of ytPlaylist.items) {
            if (item instanceof YTNodes.PlaylistVideo) {
              const parsed = parseSong(item);
              if (parsed) songs.push(parsed);
            }
          }
          return new RequestedPlaylistFromUrlResponse({ songs });
        } catch {
          return new RequestedPlaylistFromUrlResponse();
        }
      }
    });

    api.on("requestedPlaylistSongs", async ({ id }) => {
      if (!this.innertube || !id) {
        return new RequestedPlaylistSongsResponse();
      }
      const playlistId = sanitizeId(id);
      try {
        const playlist = await this.innertube.music.getPlaylist(playlistId);
        const songs: Song[] = [];

        for (const item of playlist.items) {
          if (item instanceof YTNodes.MusicResponsiveListItem) {
            const parsed = parseSong(item);
            if (parsed) songs.push(parsed);
          }
        }

        return new RequestedPlaylistSongsResponse({
          songs,
        });
      } catch (e) {
        console.error("failed to get playlist songs", e);
        return new RequestedPlaylistSongsResponse();
      }
    });

    api.on("requestedArtistSongs", async ({ artist }) => {
      if (!this.innertube || !artist?.artistId) {
        return new RequestedArtistSongsResponse();
      }
      const artistId = sanitizeId(artist.artistId);
      try {
        const ytArtist = await this.innertube.music.getArtist(artistId);
        const songs: Song[] = [];

        try {
          const songsShelf = await ytArtist.getAllSongs();
          if (songsShelf) {
            for (const item of songsShelf.contents) {
              if (item instanceof YTNodes.MusicResponsiveListItem) {
                const parsed = parseSong(item);
                if (parsed) songs.push(parsed);
              }
            }
          }
        } catch {
          for (const sec of ytArtist.sections) {
            if (sec instanceof YTNodes.MusicShelf || sec instanceof YTNodes.MusicCarouselShelf) {
              for (const item of sec.contents) {
                if (item instanceof YTNodes.MusicResponsiveListItem || item instanceof YTNodes.MusicTwoRowItem) {
                  const parsed = parseSong(item);
                  if (parsed) songs.push(parsed);
                }
              }
            }
          }
        }

        return new RequestedArtistSongsResponse({
          songs,
        });
      } catch (e) {
        console.error("failed to get artist songs", e);
        return new RequestedArtistSongsResponse();
      }
    });

    api.on("requestedAlbumSongs", async ({ album }) => {
      if (!this.innertube || !album?.albumId) {
        return new RequestedAlbumSongsResponse();
      }
      const albumId = sanitizeId(album.albumId);
      try {
        const ytAlbum = await this.innertube.music.getAlbum(albumId);
        const songs = ytAlbum.contents.map(parseSong).filter(Boolean);
        return new RequestedAlbumSongsResponse({ songs });
      } catch (e) {
        console.error("failed to get album songs", e);
        return new RequestedAlbumSongsResponse();
      }
    });

    api.on("requestedRecommendations", async () => {
      if (!this.innertube) {
        return new RequestedRecommendationsResponse();
      }
      const songs: Song[] = [];
      try {
        if (this.innertube.session.logged_in) {
          const home = await this.innertube.music.getHomeFeed();
          for (const sec of home.sections ?? []) {
            if (sec instanceof YTNodes.MusicCarouselShelf) {
              for (const item of sec.contents) {
                if (item instanceof YTNodes.MusicResponsiveListItem || item instanceof YTNodes.MusicTwoRowItem) {
                  const parsed = parseSong(item);
                  if (parsed) songs.push(parsed);
                }
              }
            }
          }
        } else {
          const explore = await this.innertube.music.getExplore();
          for (const sec of explore.sections) {
            for (const item of sec.contents) {
              if (item instanceof YTNodes.MusicResponsiveListItem || item instanceof YTNodes.MusicTwoRowItem) {
                const parsed = parseSong(item);
                if (parsed) songs.push(parsed);
              }
            }
          }
        }
      } catch (e) {
        console.error("failed to get recommendations", e);
      }
      return new RequestedRecommendationsResponse({ songs });
    });

    api.on("requestedPlaylists", async () => {
      if (!this.innertube || !this.innertube.session.logged_in) {
        return new RequestedPlaylistsResponse();
      }
      try {
        const feed = await this.innertube.getPlaylists();
        const playlists = feed.playlists.map(parsePlaylist).filter(Boolean);
        return new RequestedPlaylistsResponse({ playlists });
      } catch (e) {
        console.error("failed to get playlists", e);
        return new RequestedPlaylistsResponse();
      }
    });

    api.on("requestedSearchResult", async ({ query }) => {
      if (!this.innertube) {
        return new RequestedSearchResultResponse();
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

      return new RequestedSearchResultResponse({
        songs: songSearchRes.songs?.contents.map(parseSong).filter(Boolean),
        playlists: playlistSearchRes.playlists?.contents
          .map(parsePlaylist)
          .filter(Boolean),
        artists: artistSearchRes.artists?.contents
          .map(parseArtist)
          .filter(Boolean),
        albums: albumSearchRes.albums?.contents.map(parseAlbum).filter(Boolean),
      });
    });
  }
}

export function entry(): number {
  extension = new YoutubeJsExtension();
  extension.init();
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
