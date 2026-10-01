import {
  GetProviderScopesResponse,
  RequestedSearchResultResponse,
  getApi,
  ExtensionProviderScope,
} from "wasm-extension-js";
import { parseAlbum, parseArtist, parsePlaylist, parseSong } from "./model";

export { handle_extension_command } from "wasm-extension-js";
import { Innertube } from "youtubei.js";

let extension: YoutubeJsExtension;
const api = getApi();

class YoutubeJsExtension {
  private innertube?: Innertube;

  public async init() {
    this.register_api();
    if (typeof (globalThis as any).Intl === "undefined") {
      (globalThis as any).Intl = {
        DateTimeFormat: () => ({
          resolvedOptions: () => ({ timeZone: "UTC" }),
        }),
      };
    }

    try {
      this.innertube = await Innertube.create({
        timezone: "UTC",
        generate_session_locally: true,
        // retrieve_player: false,
        // enable_session_cache: false,
        // retrieve_innertube_config: false,
        // fetch: async (input, init) => {
        //   const resp = await api.fetch(input, init);
        //   const result = await resp.text();
        //   console.log("resp.text function source:\n", Response.toString());
        //   console.log("typeof result:", typeof result, "value:", result);
        //   return resp;
        // },
      });
      console.log("created innertube", this.innertube);
    } catch (e) {
      console.error("failed to create innertube", e);
    }
  }

  private register_api() {
    api.on(
      "getProviderScopes",
      (_req) =>
        new GetProviderScopesResponse({
          scopes: [ExtensionProviderScope.SEARCH],
        }),
    );

    api.on("requestedSearchResult", async ({ query }) => {
      console.log("inside search", typeof this.innertube);
      if (!this.innertube) {
        return new RequestedSearchResultResponse();
      }
      console.log("calling search");
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

      // console.log("got search res", searchRes);

      if ((songSearchRes.songs?.contents.length ?? 0) < 10) {
        const searchRes2 = await songSearchRes.getMore(songSearchRes.songs);
        songSearchRes.albums?.contents.push(...searchRes2.albums?.contents);
        songSearchRes.songs?.contents.push(...searchRes2.songs?.contents);
        songSearchRes.artists?.contents.push(...searchRes2.artists?.contents);
        songSearchRes.playlists?.contents.push(
          ...searchRes2.playlists?.contents,
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
