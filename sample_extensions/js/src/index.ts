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
    return new CustomRequestResponse();
  });

  return 0;
}