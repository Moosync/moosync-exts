from moosync_edk import Extension, register_extension
from core.types.protos import extensions_pb2, songs_pb2


def entry():
    print("calling entry")
    register_extension(SampleExtension())


class SampleExtension(Extension):
    def get_provider_scopes(self, req):
        return extensions_pb2.GetProviderScopesResponse(
            scopes=[extensions_pb2.ExtensionProviderScope.ACCOUNTS]
        )

    def get_accounts(self, req):
        self.api.update_accounts("sample.pkg")
        return extensions_pb2.GetAccountsResponse(
            accounts=[
                extensions_pb2.ExtensionAccountDetail(
                    id="test_account",
                    name="Test Account",
                    logged_in=True,
                )
            ]
        )

    def perform_account_login(self, req):
        self.api.register_oauth("https://example.com/callback")
        return extensions_pb2.PerformAccountLoginResponse(status="success")

    def handle_custom_request(self, req):
        if req.request_id == "hash_test":
            return extensions_pb2.CustomRequestResponse()

        if req.request_id == "preferences_test":
            self.api.register_user_preferences([])
            self.api.unregister_user_preferences([])
            return extensions_pb2.CustomRequestResponse()

        return extensions_pb2.CustomRequestResponse()

    def get_search(self, req):
        self.api.open_external_url("test")
        return extensions_pb2.RequestedSearchResultResponse(songs=[])

    def on_context_menu_action(self, req):
        if req.action_id == "add_test":
            self.api.add_playlist(songs_pb2.Playlist())
            self.api.add_songs([])
            self.api.add_to_playlist("id", [])
        return extensions_pb2.ContextMenuActionResponse()

    def on_preferences_changed(self, req):
        self.api.get_preference(extensions_pb2.PreferenceData(key="test"))
        self.api.get_secure(extensions_pb2.PreferenceData(key="test"))
        return extensions_pb2.PreferenceChangedResponse()

    def on_queue_changed(self, req):
        self.api.get_queue()
        return extensions_pb2.SongQueueChangedResponse()

    def on_volume_changed(self, req):
        self.api.get_volume()
        return extensions_pb2.VolumeChangedResponse()

    def on_player_state_changed(self, req):
        self.api.get_player_state()
        return extensions_pb2.PlayerStateChangedResponse()

    def on_song_changed(self, req):
        self.api.get_current_song()
        return extensions_pb2.SongChangedResponse()

    def on_seeked(self, req):
        self.api.get_time()
        return extensions_pb2.SeekedResponse()
