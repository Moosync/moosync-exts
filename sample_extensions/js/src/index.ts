import {
  ExtensionAccountDetail,
  GetAccountsResponse,
  GetProviderScopesResponse,
  PerformAccountLoginResponse,
  getApi,
  Playlist,
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
  GetAccountsRequest,
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

// Initialize extension
// console.log("Sample extension loaded");

export function entry(): number {
  const api = getApi();

  api.on("getProviderScopes", () => {
    return new GetProviderScopesResponse({
      scopes: [
        13 as any
      ]
    });
  });

  api.on("getAccounts", () => {
    api.updateAccounts("sample.pkg");
    const account = new ExtensionAccountDetail({
      id: "test_account",
      name: "Test Account",
      loggedIn: true,
      username: "User",
      bgColor: "",
      icon: ""
    });
    return new GetAccountsResponse({
      accounts: [account]
    });
  });

  api.on("performAccountLogin", () => {
    api.registerOauth("https://example.com/callback");
    return new PerformAccountLoginResponse({
      status: "success"
    });
  });

  api.on("requestedSearchResult", () => {
    api.openExternalUrl("https://example.com");
    return new RequestedSearchResultResponse({
      songs: []
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