import {
  ExtensionAccountDetail,
  ExtensionProviderScope,
  GetProviderScopesResponse,
  PerformAccountLoginResponse,
  getApi,
  Playlist,
  Song,
  InnerSong,
  Artist,
  Album,
  RequestedPlaylistsResponse,
  RequestedPlaylistSongsResponse,
  RequestedArtistSongsResponse,
  RequestedAlbumSongsResponse,
  RequestedSearchResultResponse,
  SongChangedResponse,
  PlayerStateChangedResponse,
  VolumeChangedResponse,
  SongQueueChangedResponse,
  SeekedResponse,
  PreferenceChangedResponse,
  ContextMenuActionResponse,
  CustomRequestResponse,
  PreferenceData,
  AddToPlaylistRequest,
  GetProviderScopesRequest,
  PerformAccountLoginRequest,
  RequestedSearchResultRequest,
  SongChangedRequest,
  PlayerStateChangedRequest,
  VolumeChangedRequest,
  SongQueueChangedRequest,
  SeekedRequest,
  PreferenceChangedRequest,
  ContextMenuActionRequest,
  CustomRequest,
} from "wasm-extension-js";

export { handle_extension_command } from "wasm-extension-js";

function mockSampleSong(id: string, title: string): Song {
  return new Song({
    song: new InnerSong({
      id,
      title,
    }),
    artists: [
      new Artist({
        artistId: `art-${id}`,
        artistName: `Artist for ${title}`,
      }),
    ],
    album: new Album({
      albumId: `alb-${id}`,
      albumName: `Album for ${title}`,
    }),
  });
}

// Initialize extension
// console.log("Sample extension loaded");

export function entry(): number {
  const api = getApi();

  api.on("getProviderScopes", () => {
    return new GetProviderScopesResponse({
      scopes: [
        ExtensionProviderScope.ACCOUNTS,
        ExtensionProviderScope.SEARCH,
        ExtensionProviderScope.PLAYLISTS,
        ExtensionProviderScope.PLAYLIST_SONGS,
        ExtensionProviderScope.ARTIST_SONGS,
        ExtensionProviderScope.ALBUM_SONGS,
      ]
    });
  });

  api.on("requestedPlaylists", () => {
    return new RequestedPlaylistsResponse({
      playlists: [
        new Playlist({
          playlistId: "ext-playlist-1",
          playlistName: "Extension Playlist 1",
        }),
        new Playlist({
          playlistId: "ext-playlist-2",
          playlistName: "Extension Playlist 2",
        }),
      ]
    });
  });

  api.on("requestedPlaylistSongs", () => {
    return new RequestedPlaylistSongsResponse({
      songs: [
        mockSampleSong("pl-song-1", "Playlist Song 1"),
        mockSampleSong("pl-song-2", "Playlist Song 2"),
      ]
    });
  });

  api.on("requestedArtistSongs", () => {
    return new RequestedArtistSongsResponse({
      songs: [
        mockSampleSong("artist-song-1", "Artist Song 1"),
        mockSampleSong("artist-song-2", "Artist Song 2"),
      ]
    });
  });

  api.on("requestedAlbumSongs", () => {
    return new RequestedAlbumSongsResponse({
      songs: [
        mockSampleSong("album-song-1", "Album Song 1"),
        mockSampleSong("album-song-2", "Album Song 2"),
      ]
    });
  });

  let loggedIn = false;

  api.on("performAccountLogin", (req: PerformAccountLoginRequest) => {
    api.registerOauth("https://example.com/callback");
    if (req.loginStatus) {
      loggedIn = true;
      api.setAccount(
        new ExtensionAccountDetail({
          id: "test_account",
          name: "Test Account",
          loggedIn: true,
          username: "User",
          bgColor: "",
          icon: ""
        })
      );
      return new PerformAccountLoginResponse({
        status: "success"
      });
    } else {
      loggedIn = false;
      api.setAccount(
        new ExtensionAccountDetail({
          id: "test_account",
          name: "Test Account",
          loggedIn: false,
          username: "",
          bgColor: "",
          icon: ""
        })
      );
      return new PerformAccountLoginResponse({
        status: "logged_out"
      });
    }
  });

  api.setAccount(
    new ExtensionAccountDetail({
      id: "test_account",
      name: "Test Account",
      loggedIn: false,
      username: "",
      bgColor: "",
      icon: ""
    })
  );

  api.on("requestedSearchResult", (req: RequestedSearchResultRequest) => {
    api.openExternalUrl("https://example.com");
    if (req?.query?.trim().toLowerCase() === "logged in" && !loggedIn) {
      return new RequestedSearchResultResponse({
        songs: [],
        playlists: [],
        artists: [],
        albums: [],
      });
    }
    return new RequestedSearchResultResponse({
      songs: [
        mockSampleSong("search-song-1", "Search Song 1"),
        mockSampleSong("search-song-2", "Search Song 2"),
      ],
      playlists: [
        new Playlist({
          playlistId: "search-pl-1",
          playlistName: "Search Playlist 1",
        }),
        new Playlist({
          playlistId: "search-pl-2",
          playlistName: "Search Playlist 2",
        }),
      ],
      artists: [
        new Artist({
          artistId: "search-art-1",
          artistName: "Search Artist 1",
        }),
        new Artist({
          artistId: "search-art-2",
          artistName: "Search Artist 2",
        }),
      ],
      albums: [
        new Album({
          albumId: "search-alb-1",
          albumName: "Search Album 1",
        }),
        new Album({
          albumId: "search-alb-2",
          albumName: "Search Album 2",
        }),
      ],
    });
  });

  api.on("songChanged", () => {
    api.getCurrentSong();
    return new SongChangedResponse();
  });

  api.on("playerStateChanged", () => {
    api.getPlayerState();
    return new PlayerStateChangedResponse();
  });

  api.on("volumeChanged", () => {
    api.getVolume();
    return new VolumeChangedResponse();
  });

  api.on("songQueueChanged", () => {
    api.getQueue();
    return new SongQueueChangedResponse();
  });

  api.on("seeked", () => {
    api.getTime();
    return new SeekedResponse();
  });

  api.on("preferenceChanged", () => {
    api.getPreference(new PreferenceData({ key: "test_key" }));
    api.getSecure(new PreferenceData({ key: "test_key" }));
    return new PreferenceChangedResponse();
  });

  api.on("contextMenuAction", () => {
    api.addPlaylist(new Playlist());
    api.addSongs([]);
    api.addToPlaylist(new AddToPlaylistRequest());
    return new ContextMenuActionResponse();
  });

  api.on("customRequest", (req) => {
    if (req.requestId === "preferences_test") {
      api.registerUserPreferences([]);
      api.unregisterUserPreferences([]);
    }
    if (req.requestId === "http_get_test") {
      api.fetch("https://example.com");
    }
    if (req.requestId === "http_request_test") {
      api.fetch({
        url: "https://example.com",
        method: "POST",
        headers: { "X-Test": "Value" },
        body: new Uint8Array([1, 2, 3]),
        timeoutMs: 5000,
      });
    }
    if (req.requestId === "http_batch_get_test") {
      api.batchFetch(["https://example.com/1", "https://example.com/2"]);
    }
    if (req.requestId === "http_batch_request_test") {
      api.batchFetch([
        { url: "https://example.com/1", method: "GET" },
        { url: "https://example.com/2", method: "POST", body: new Uint8Array([4, 5, 6]) },
      ]);
    }
    return new CustomRequestResponse();
  });

  return 0;
}