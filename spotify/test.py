# pylint: disable=no-member,unused-import,too-many-locals,too-many-branches,too-many-nested-blocks,too-many-statements,protected-access,too-many-lines
import json
import os
import sys
import time
import pytest
from core.types.protos.extensions_pb2 import (
    BatchHttpRequest,
    BatchHttpResponse,
    CanPlayRequest,
    ExtensionAccountDetail,
    ExtensionCommand,
    ExtensionCommandResponse,
    GetCacheRequest,
    GetCacheResponse,
    GetPreferenceRequest,
    GetPreferenceResponse,
    GetProcessStatusRequest,
    GetProcessStatusResponse,
    GetSecureRequest,
    GetSecureResponse,
    HttpRequest,
    HttpResponse,
    HttpResult,
    MainCommand,
    MainCommandResponse,
    OauthCallbackRequest,
    PerformAccountLoginRequest,
    PlayerLoadTrackRequest,
    PlayerLoadTrackResponse,
    PlayerPauseRequest,
    PlayerPauseResponse,
    PlayerPlayRequest,
    PlayerPlayResponse,
    PlayerSeekRequest,
    PlayerSeekResponse,
    PlayerSetVolumeRequest,
    PlayerSetVolumeResponse,
    PlayerStopRequest,
    PlayerStopResponse,
    PreferenceData,
    RegisterOauthRequest,
    RegisterOauthResponse,
    RegisterUserPreferenceRequest,
    RegisterUserPreferenceResponse,
    RequestedAlbumSongsRequest,
    RequestedArtistSongsRequest,
    RequestedPlaylistSongsRequest,
    RequestedPlaylistsRequest,
    RequestedRecommendationsRequest,
    RequestedSearchResultRequest,
    RequestedSongFromIdRequest,
    SetAccountRequest,
    SetAccountResponse,
    SetSecureRequest,
    SetSecureResponse,
)
from core.types.protos.preferences_pb2 import (
    PreferenceItem,
    PreferenceType,
    PreferenceValue,
    StatusDot,
    StatusDotColor,
    StatusDotList,
)
from core.types.protos.songs_pb2 import (
    Album,
    Artist,
    InnerSong,
    Playlist,
    SearchResult,
    Song,
)
from google.protobuf.duration_pb2 import Duration
from google.protobuf.struct_pb2 import Struct, Value
from moounit import Moounit


def load_env() -> dict[str, str]:
    env_vars: dict[str, str] = {}
    candidates = [
        os.path.join(os.path.dirname(__file__), ".env"),
        os.path.join(
            os.environ.get("BUILD_WORKSPACE_DIRECTORY", ""), "spotify", ".env"
        ),
        ".env",
    ]
    for candidate in candidates:
        if candidate and os.path.isfile(candidate):
            with open(candidate, "r", encoding="utf-8") as f:
                for line in f:
                    line = line.strip()
                    if line and not line.startswith("#") and "=" in line:
                        k, v = line.split("=", 1)
                        env_vars[k.strip()] = v.strip().strip("'\"")
            break
    return env_vars


_ENV = load_env()
IS_RECORD_MODE = "--record" in sys.argv

if IS_RECORD_MODE:
    CLIENT_ID = _ENV.get("SPOTIFY_CLIENT_ID", os.environ.get("SPOTIFY_CLIENT_ID", ""))
    CLIENT_SECRET = _ENV.get(
        "SPOTIFY_CLIENT_SECRET", os.environ.get("SPOTIFY_CLIENT_SECRET", "")
    )
    ACCESS_TOKEN = _ENV.get(
        "SPOTIFY_ACCESS_TOKEN",
        os.environ.get("SPOTIFY_ACCESS_TOKEN", "dummy_spotify_access_token"),
    )
    REFRESH_TOKEN = _ENV.get(
        "SPOTIFY_REFRESH_TOKEN",
        os.environ.get("SPOTIFY_REFRESH_TOKEN", "dummy_spotify_refresh_token"),
    )
    SOLOIST_API_KEY = _ENV.get(
        "SPOTIFY_SOLOIST_API_KEY", os.environ.get("SPOTIFY_SOLOIST_API_KEY", "")
    )
else:
    CLIENT_ID = "dummy_spotify_client_id"
    CLIENT_SECRET = "dummy_spotify_client_secret"
    ACCESS_TOKEN = "dummy_spotify_access_token"
    REFRESH_TOKEN = "dummy_spotify_refresh_token"
    SOLOIST_API_KEY = "dummy_soloist_api_key"


def update_env_file(access_token: str, refresh_token: str):
    env_path = os.path.join(
        os.environ.get("BUILD_WORKSPACE_DIRECTORY", ""), "spotify", ".env"
    )
    if not os.path.exists(env_path):
        env_path = os.path.join(os.path.dirname(__file__), ".env")
    lines = []
    if os.path.exists(env_path):
        with open(env_path, "r", encoding="utf-8") as f:
            for line in f:
                if not (
                    line.startswith("SPOTIFY_ACCESS_TOKEN=")
                    or line.startswith("SPOTIFY_REFRESH_TOKEN=")
                ):
                    lines.append(line.rstrip("\n"))
    lines.append(f"SPOTIFY_ACCESS_TOKEN={access_token}")
    lines.append(f"SPOTIFY_REFRESH_TOKEN={refresh_token}")
    with open(env_path, "w", encoding="utf-8") as f:
        f.write("\n".join(lines) + "\n")


@pytest.fixture
def entry(moounit: Moounit):
    moounit.expect_command(
        MainCommand(
            get_secure=GetSecureRequest(data=PreferenceData(key="client_secret"))
        ),
        MainCommandResponse(get_secure=GetSecureResponse(data=PreferenceData())),
    )
    moounit.expect_command(
        MainCommand(get_secure=GetSecureRequest(data=PreferenceData(key="client_id"))),
        MainCommandResponse(get_secure=GetSecureResponse(data=PreferenceData())),
    )
    moounit.expect_command(
        MainCommand(get_secure=GetSecureRequest(data=PreferenceData(key="tokens"))),
        MainCommandResponse(get_secure=GetSecureResponse(data=PreferenceData())),
    )
    moounit.expect_command(
        MainCommand(
            get_preference=GetPreferenceRequest(
                data=PreferenceData(key="soloist_ws_port")
            )
        ),
        MainCommandResponse(
            get_preference=GetPreferenceResponse(data=PreferenceData())
        ),
    )
    moounit.expect_command(
        MainCommand(
            get_preference=GetPreferenceRequest(
                data=PreferenceData(key="client_secret")
            )
        ),
        MainCommandResponse(
            get_preference=GetPreferenceResponse(data=PreferenceData())
        ),
        times=-1,
    )
    moounit.expect_command(
        MainCommand(
            get_preference=GetPreferenceRequest(data=PreferenceData(key="client_id"))
        ),
        MainCommandResponse(
            get_preference=GetPreferenceResponse(data=PreferenceData())
        ),
        times=-1,
    )
    try:
        moounit.call_entry()
    except Exception as e:
        print(f"Failed to call entry: {e}")
        # Re-raise to fail the test and see logs
        raise e
    return moounit


@pytest.fixture
def configured_entry(moounit: Moounit):
    moounit.expect_command(
        MainCommand(
            get_secure=GetSecureRequest(data=PreferenceData(key="client_secret"))
        ),
        MainCommandResponse(
            get_secure=GetSecureResponse(
                data=PreferenceData(
                    key="client_secret",
                    value=Value(string_value=CLIENT_SECRET),
                )
            )
        ),
    )
    moounit.expect_command(
        MainCommand(get_secure=GetSecureRequest(data=PreferenceData(key="client_id"))),
        MainCommandResponse(
            get_secure=GetSecureResponse(
                data=PreferenceData(
                    key="client_id",
                    value=Value(string_value=CLIENT_ID),
                )
            )
        ),
    )
    moounit.expect_command(
        MainCommand(get_secure=GetSecureRequest(data=PreferenceData(key="tokens"))),
        MainCommandResponse(get_secure=GetSecureResponse(data=PreferenceData())),
    )
    moounit.expect_command(
        MainCommand(
            get_preference=GetPreferenceRequest(
                data=PreferenceData(key="soloist_ws_port")
            )
        ),
        MainCommandResponse(
            get_preference=GetPreferenceResponse(data=PreferenceData())
        ),
    )
    moounit.expect_command(
        MainCommand(
            get_preference=GetPreferenceRequest(
                data=PreferenceData(key="client_secret")
            )
        ),
        MainCommandResponse(
            get_preference=GetPreferenceResponse(data=PreferenceData())
        ),
        times=-1,
    )
    moounit.expect_command(
        MainCommand(
            get_preference=GetPreferenceRequest(data=PreferenceData(key="client_id"))
        ),
        MainCommandResponse(
            get_preference=GetPreferenceResponse(data=PreferenceData())
        ),
        times=-1,
    )
    try:
        moounit.call_entry()
    except Exception as e:
        print(f"Failed to call configured entry: {e}")
        raise e
    return moounit


@pytest.fixture
def authed_entry(moounit: Moounit, request: pytest.FixtureRequest):
    tokens_struct = Struct()
    tokens_struct["access_token"] = ACCESS_TOKEN
    tokens_struct["refresh_token"] = REFRESH_TOKEN
    tokens_struct["expires_in"] = 3600
    tokens_struct["expires_at"] = 2500000000000
    tokens_struct["scope"] = "user-read-private"

    moounit.replay_ignore(
        BatchHttpRequest(),
        ignore_fields=["requests.headers"],
    )
    moounit.replay_ignore(
        MainCommand(),
        ignore_fields=[
            "spawn_process.env.SPOTIFY_SOLOIST_API_KEY",
            "register_user_preference.prefs.value",
        ],
    )

    moounit.expect_command(
        MainCommand(
            get_secure=GetSecureRequest(data=PreferenceData(key="client_secret"))
        ),
        MainCommandResponse(
            get_secure=GetSecureResponse(
                data=PreferenceData(
                    key="client_secret",
                    value=Value(string_value=CLIENT_SECRET),
                )
            )
        ),
    )
    moounit.expect_command(
        MainCommand(get_secure=GetSecureRequest(data=PreferenceData(key="client_id"))),
        MainCommandResponse(
            get_secure=GetSecureResponse(
                data=PreferenceData(
                    key="client_id",
                    value=Value(string_value=CLIENT_ID),
                )
            )
        ),
    )
    moounit.expect_command(
        MainCommand(get_secure=GetSecureRequest(data=PreferenceData(key="tokens"))),
        MainCommandResponse(
            get_secure=GetSecureResponse(
                data=PreferenceData(
                    key="tokens",
                    value=Value(struct_value=tokens_struct),
                )
            )
        ),
    )
    moounit.expect_command(
        MainCommand(
            get_preference=GetPreferenceRequest(
                data=PreferenceData(key="soloist_ws_port")
            )
        ),
        MainCommandResponse(
            get_preference=GetPreferenceResponse(
                data=PreferenceData(
                    key="soloist_ws_port",
                    value=Value(string_value="9090"),
                )
            )
        ),
    )
    if request.node.name in ("test_soloist_pairing", "test_soloist_playback"):
        moounit.expect_command(
            MainCommand(
                get_preference=GetPreferenceRequest(
                    data=PreferenceData(key="soloist_api_key")
                )
            ),
            MainCommandResponse(
                get_preference=GetPreferenceResponse(
                    data=PreferenceData(
                        key="soloist_api_key",
                        value=Value(string_value=SOLOIST_API_KEY),
                    )
                )
            ),
            times=-1,
        )
    if request.node.name == "test_soloist_playback":
        moounit.expect_command(
            MainCommand(get_process_status=GetProcessStatusRequest(process_id=1)),
            MainCommandResponse(
                get_process_status=GetProcessStatusResponse(is_running=True)
            ),
            times=-1,
        )
    elif request.node.name != "test_soloist_pairing":
        moounit.expect_command(
            MainCommand(
                get_preference=GetPreferenceRequest(
                    data=PreferenceData(key="soloist_api_key")
                )
            ),
            MainCommandResponse(
                get_preference=GetPreferenceResponse(
                    data=PreferenceData(
                        key="soloist_api_key",
                        value=Value(string_value=SOLOIST_API_KEY),
                    )
                )
            ),
            times=10,
        )
    moounit.expect_command(
        MainCommand(
            get_secure=GetSecureRequest(data=PreferenceData(key="soloist_api_key"))
        ),
        MainCommandResponse(get_secure=GetSecureResponse(data=PreferenceData())),
        times=-1,
    )
    soloist_url = "https://soloist-builds.spotifycdn.com/soloist_release_x86_64.tar.gz"
    if request.node.name not in ("test_soloist_pairing", "test_soloist_playback"):
        moounit.expect_command(
            MainCommand(
                batch_http_request=BatchHttpRequest(
                    requests=[HttpRequest(url=soloist_url, method="GET")]
                )
            ),
            MainCommandResponse(
                batch_http_request=BatchHttpResponse(
                    responses=[
                        HttpResult(response=HttpResponse(status_code=404, body=b""))
                    ]
                )
            ),
            times=-1,
        )
    moounit.expect_command(
        MainCommand(
            get_preference=GetPreferenceRequest(
                data=PreferenceData(key="client_secret")
            )
        ),
        MainCommandResponse(
            get_preference=GetPreferenceResponse(data=PreferenceData())
        ),
        times=-1,
    )
    moounit.expect_command(
        MainCommand(
            get_preference=GetPreferenceRequest(data=PreferenceData(key="client_id"))
        ),
        MainCommandResponse(
            get_preference=GetPreferenceResponse(data=PreferenceData())
        ),
        times=-1,
    )
    try:
        moounit.call_entry()
    except Exception as e:
        print(f"Failed to call authed entry: {e}")
        raise e
    return moounit


def test_get_provider_scopes(entry: Moounit):
    scopes = list(entry.manifest.scopes)
    assert len(scopes) == 10


def test_can_play_non_spotify(entry: Moounit):
    ret = entry.send_command(
        ExtensionCommand(
            can_play=CanPlayRequest(
                src="https://www.youtube.com/watch?v=123",
                song=Song(song=InnerSong(id="youtube:123")),
            )
        )
    )
    assert ret.HasField("can_play")
    assert ret.can_play.can_play is False


def test_can_play_not_premium(entry: Moounit):
    ret = entry.send_command(
        ExtensionCommand(
            can_play=CanPlayRequest(
                src="app.moosync.spotify:4uLU6hMCjMI75M1A2tKUQC",
                song=Song(song=InnerSong(id="app.moosync.spotify:4uLU6hMCjMI75M1A2tKUQC")),
            )
        )
    )
    assert ret.HasField("can_play")
    assert ret.can_play.can_play is False


def test_search_cache_hit(entry: Moounit):
    cached_search = SearchResult(
        songs=[
            Song(
                song=InnerSong(
                    id="spotify:cached_spotify_1",
                    title="Cached Spotify Song",
                )
            )
        ]
    )
    entry.expect_command(
        command=MainCommand(get_cache=GetCacheRequest(key="search:electronic")),
        response=MainCommandResponse(
            get_cache=GetCacheResponse(data=cached_search.SerializeToString())
        ),
    )

    ret = entry.send_command(
        ExtensionCommand(
            requested_search_result=RequestedSearchResultRequest(query="electronic")
        )
    )

    assert ret.HasField("requested_search_result")
    assert len(ret.requested_search_result.songs) == 1
    assert ret.requested_search_result.songs[0].song.title == "Cached Spotify Song"


def test_player_methods_unauthorized(entry: Moounit):
    entry.expect_command(
        MainCommand(
            get_secure=GetSecureRequest(data=PreferenceData(key="soloist_api_key"))
        ),
        MainCommandResponse(get_secure=GetSecureResponse(data=PreferenceData())),
    )
    entry.expect_command(
        MainCommand(
            get_preference=GetPreferenceRequest(
                data=PreferenceData(key="soloist_api_key")
            )
        ),
        MainCommandResponse(
            get_preference=GetPreferenceResponse(data=PreferenceData())
        ),
    )
    can_ret = entry.send_command(
        ExtensionCommand(
            can_play=CanPlayRequest(
                src="spotify:track:4uLU6hMCjMI75M1A2tKUQC",
                song=Song(song=InnerSong(id="app.moosync.spotify:4uLU6hMCjMI75M1A2tKUQC")),
            )
        )
    )
    assert can_ret.can_play.can_play is False

    with pytest.raises(Exception) as exc_info:
        entry.send_command(
            ExtensionCommand(
                player_load_track=PlayerLoadTrackRequest(
                    src="spotify:track:4uLU6hMCjMI75M1A2tKUQC",
                    song=Song(
                        song=InnerSong(id="app.moosync.spotify:4uLU6hMCjMI75M1A2tKUQC")
                    ),
                )
            )
        )
    assert "Spotify Premium is required" in str(exc_info.value)

    with pytest.raises(Exception) as exc_info:
        entry.send_command(ExtensionCommand(player_play=PlayerPlayRequest()))
    assert "Soloist WebSocket not connected" in str(exc_info.value)

    with pytest.raises(Exception) as exc_info:
        entry.send_command(ExtensionCommand(player_pause=PlayerPauseRequest()))
    assert "Soloist WebSocket not connected" in str(exc_info.value)

    with pytest.raises(Exception) as exc_info:
        entry.send_command(ExtensionCommand(player_stop=PlayerStopRequest()))
    assert "Soloist WebSocket not connected" in str(exc_info.value)

    with pytest.raises(Exception) as exc_info:
        entry.send_command(
            ExtensionCommand(
                player_seek=PlayerSeekRequest(position=Duration(seconds=15))
            )
        )
    assert "Soloist WebSocket not connected" in str(exc_info.value)

    with pytest.raises(Exception) as exc_info:
        entry.send_command(
            ExtensionCommand(player_set_volume=PlayerSetVolumeRequest(volume=50))
        )
    assert "Soloist WebSocket not connected" in str(exc_info.value)


def test_soloist_pairing(authed_entry: Moounit):
    can_ret1 = authed_entry.send_command(
        ExtensionCommand(
            can_play=CanPlayRequest(
                src="spotify:track:4uLU6hMCjMI75M1A2tKUQC",
                song=Song(song=InnerSong(id="app.moosync.spotify:4uLU6hMCjMI75M1A2tKUQC")),
            )
        )
    )
    assert can_ret1.HasField("can_play")
    assert can_ret1.can_play.can_play is False

    if authed_entry.recorder.record_mode:
        assert (
            authed_entry.process_manager.processes
        ), "Soloist pairing process was not spawned"
        print("\n" + "=" * 60, flush=True)
        print(">>> SOLOIST IS ACTIVE IN PAIRING MODE ('Moosync') <<<", flush=True)
        print(
            "Please open Spotify app on your phone/desktop and select "
            "'Moosync' in Connect devices to pair.",
            flush=True,
        )
        print("Waiting for Soloist pairing process to complete...", flush=True)
        print("=" * 60, flush=True)
        start_time = time.time()
        while time.time() - start_time < 300:
            time.sleep(1)
            if (
                hasattr(authed_entry, "process_manager")
                and authed_entry.process_manager
            ):
                running_procs = [
                    p
                    for p in authed_entry.process_manager.processes.values()
                    if p.poll() is None
                ]
                if not running_procs and authed_entry.process_manager.processes:
                    exit_codes = [
                        p.poll()
                        for p in authed_entry.process_manager.processes.values()
                    ]
                    print(
                        f">>> Soloist process finished with exit codes: {exit_codes}",
                        flush=True,
                    )
                    break
    else:
        data_dir = authed_entry._mapped_paths.get("/cache/soloist_data")
        if data_dir:
            os.makedirs(data_dir, exist_ok=True)
            with open(
                os.path.join(data_dir, "session.json"), "w", encoding="utf-8"
            ) as f:
                f.write("{}")

    # User paired soloist
    can_ret2 = authed_entry.send_command(
        ExtensionCommand(
            can_play=CanPlayRequest(
                src="spotify:track:4uLU6hMCjMI75M1A2tKUQC",
                song=Song(song=InnerSong(id="app.moosync.spotify:4uLU6hMCjMI75M1A2tKUQC")),
            )
        )
    )
    assert can_ret2.HasField("can_play")
    assert can_ret2.can_play.can_play is True


def test_login_flow(configured_entry: Moounit):
    ret = configured_entry.send_command(
        ExtensionCommand(
            perform_account_login=PerformAccountLoginRequest(login_status=True)
        )
    )
    assert ret.HasField("perform_account_login")
    login_url = ret.perform_account_login.url
    assert "https://accounts.spotify.com/authorize" in login_url
    assert "client_id=" in login_url
    assert "response_type=code" in login_url

    if configured_entry.recorder.record_mode:
        print("\n" + "=" * 60, flush=True)
        print(">>> SPOTIFY LOGIN URL <<<", flush=True)
        print(login_url, flush=True)
        print("=" * 60, flush=True)
        print(
            "Waiting for callback code or URL in 'spotify/.auth_code'...",
            flush=True,
        )

        code_file = os.path.join(
            os.environ.get("BUILD_WORKSPACE_DIRECTORY", ""), "spotify", ".auth_code"
        )
        if not os.path.exists(os.path.dirname(code_file)):
            code_file = os.path.join(os.path.dirname(__file__), ".auth_code")
        if os.path.exists(code_file):
            os.remove(code_file)

        start_time = time.time()
        code = None
        while time.time() - start_time < 600:
            if os.path.exists(code_file):
                with open(code_file, "r", encoding="utf-8") as f:
                    content = f.read().strip()
                if content:
                    code = content
                    break
            time.sleep(1)

        assert code is not None, "Timed out waiting for Spotify auth code"
        if os.path.exists(code_file):
            os.remove(code_file)

        if "code=" in code:
            code = code.split("code=")[1].split("&")[0]
        elif "&" in code:
            code = code.split("&")[0]

        cb_ret = configured_entry.send_command(
            ExtensionCommand(oauth_callback=OauthCallbackRequest(callback_uri=code))
        )
        assert cb_ret.HasField("oauth_callback")

        calls = configured_entry.recorder.recorded_test_calls.get(
            configured_entry.recorder.current_test_name, []
        )
        for call in calls:
            req = call.get("request", {})
            if "send_main_command" in req:
                cmd = req["send_main_command"]
                if "set_secure" in cmd:
                    data = cmd["set_secure"].get("data", {})
                    if data.get("key") == "tokens":
                        val = data.get("value", {})
                        acc = val.get("access_token")
                        ref = val.get("refresh_token")
                        if acc and ref:
                            update_env_file(acc, ref)
                            print(">>> Successfully saved tokens to .env!", flush=True)


def test_search(authed_entry: Moounit):
    ret = authed_entry.send_command(
        ExtensionCommand(
            requested_search_result=RequestedSearchResultRequest(
                query="Michael Jackson"
            )
        )
    )
    assert ret.HasField("requested_search_result")
    songs = ret.requested_search_result.songs
    assert len(songs) > 0
    assert songs[0].song.id.startswith("app.moosync.spotify:")


def test_playlists(authed_entry: Moounit):
    ret = authed_entry.send_command(
        ExtensionCommand(requested_playlists=RequestedPlaylistsRequest())
    )
    assert ret.HasField("requested_playlists")
    playlists = ret.requested_playlists.playlists
    assert len(playlists) > 0
    assert playlists[0].playlist_id.startswith("app.moosync.spotify:")


def test_playlist_songs(authed_entry: Moounit):
    pl_ret = authed_entry.send_command(
        ExtensionCommand(requested_playlists=RequestedPlaylistsRequest())
    )
    assert pl_ret.HasField("requested_playlists")
    assert len(pl_ret.requested_playlists.playlists) > 0
    playlist_id = pl_ret.requested_playlists.playlists[0].playlist_id

    ret = authed_entry.send_command(
        ExtensionCommand(
            requested_playlist_songs=RequestedPlaylistSongsRequest(id=playlist_id)
        )
    )
    assert ret.HasField("requested_playlist_songs")
    assert len(ret.requested_playlist_songs.songs) > 0
    assert (
        ret.requested_playlist_songs.songs[0].song.id
        == "app.moosync.spotify:spotify:track:0QMZuNKnGppi7KW9IAGHjC"
    )


def test_album_songs(authed_entry: Moounit):
    ret = authed_entry.send_command(
        ExtensionCommand(
            requested_album_songs=RequestedAlbumSongsRequest(
                album=Album(album_id="app.moosync.spotify:4aawyAB9vmqN3uQ7FjRGTy")
            )
        )
    )
    assert ret.HasField("requested_album_songs")
    songs = ret.requested_album_songs.songs
    assert len(songs) > 0
    assert songs[0].song.id.startswith("app.moosync.spotify:")


def test_artist_songs(authed_entry: Moounit):
    ret = authed_entry.send_command(
        ExtensionCommand(
            requested_artist_songs=RequestedArtistSongsRequest(
                artist=Artist(artist_id="app.moosync.spotify:06HL4z0CvFAxyc27GXpf02")
            )
        )
    )
    assert ret.HasField("requested_artist_songs")
    songs = ret.requested_artist_songs.songs
    assert len(songs) > 0
    assert songs[0].song.id.startswith("app.moosync.spotify:")


def test_song_from_id(authed_entry: Moounit):
    ret = authed_entry.send_command(
        ExtensionCommand(
            requested_song_from_id=RequestedSongFromIdRequest(
                id="app.moosync.spotify:4uLU6hMCjMI75M1A2tKUQC"
            )
        )
    )
    assert ret.HasField("requested_song_from_id")
    assert ret.requested_song_from_id.song is not None
    assert (
        ret.requested_song_from_id.song.song.id
        == "app.moosync.spotify:spotify:track:4uLU6hMCjMI75M1A2tKUQC"
    )


def test_recommendations(authed_entry: Moounit):
    ret = authed_entry.send_command(
        ExtensionCommand(requested_recommendations=RequestedRecommendationsRequest())
    )
    assert ret.HasField("requested_recommendations")
    assert len(ret.requested_recommendations.songs) > 0


def test_soloist_playback(authed_entry: Moounit):
    can_ret = authed_entry.send_command(
        ExtensionCommand(
            can_play=CanPlayRequest(
                src="spotify:track:4uLU6hMCjMI75M1A2tKUQC",
                song=Song(song=InnerSong(id="app.moosync.spotify:4uLU6hMCjMI75M1A2tKUQC")),
            )
        )
    )
    assert can_ret.HasField("can_play")

    if not can_ret.can_play.can_play:
        if authed_entry.recorder.record_mode:
            print(
                "Please pair 'Moosync' in your Spotify app Connect devices...",
                flush=True,
            )
            start_time = time.time()
            while time.time() - start_time < 300:
                time.sleep(1)
                if (
                    hasattr(authed_entry, "process_manager")
                    and authed_entry.process_manager
                ):
                    running_procs = [
                        p
                        for p in authed_entry.process_manager.processes.values()
                        if p.poll() is None
                    ]
                    if not running_procs and authed_entry.process_manager.processes:
                        break
        else:
            data_dir = authed_entry._mapped_paths.get("/cache/soloist_data")
            if data_dir:
                os.makedirs(data_dir, exist_ok=True)
                with open(
                    os.path.join(data_dir, "session.json"), "w", encoding="utf-8"
                ) as f:
                    f.write("{}")

        can_ret2 = authed_entry.send_command(
            ExtensionCommand(
                can_play=CanPlayRequest(
                    src="spotify:track:4uLU6hMCjMI75M1A2tKUQC",
                    song=Song(
                        song=InnerSong(id="app.moosync.spotify:4uLU6hMCjMI75M1A2tKUQC")
                    ),
                )
            )
        )
        assert can_ret2.can_play.can_play is True

    load_ret = authed_entry.send_command(
        ExtensionCommand(
            player_load_track=PlayerLoadTrackRequest(
                src="spotify:track:4uLU6hMCjMI75M1A2tKUQC",
                song=Song(song=InnerSong(id="app.moosync.spotify:4uLU6hMCjMI75M1A2tKUQC")),
            )
        )
    )
    assert load_ret == ExtensionCommandResponse(
        player_load_track=PlayerLoadTrackResponse()
    )

    play_ret = authed_entry.send_command(
        ExtensionCommand(player_play=PlayerPlayRequest())
    )
    assert play_ret == ExtensionCommandResponse(player_play=PlayerPlayResponse())

    pause_ret = authed_entry.send_command(
        ExtensionCommand(player_pause=PlayerPauseRequest())
    )
    assert pause_ret == ExtensionCommandResponse(player_pause=PlayerPauseResponse())

    seek_ret = authed_entry.send_command(
        ExtensionCommand(player_seek=PlayerSeekRequest(position=Duration(seconds=30)))
    )
    assert seek_ret == ExtensionCommandResponse(player_seek=PlayerSeekResponse())

    vol_ret = authed_entry.send_command(
        ExtensionCommand(player_set_volume=PlayerSetVolumeRequest(volume=80))
    )
    assert vol_ret == ExtensionCommandResponse(
        player_set_volume=PlayerSetVolumeResponse()
    )

    load2_ret = authed_entry.send_command(
        ExtensionCommand(
            player_load_track=PlayerLoadTrackRequest(
                src="spotify:track:next_track_999",
                song=Song(song=InnerSong(id="app.moosync.spotify:next_track_999")),
            )
        )
    )
    assert load2_ret == ExtensionCommandResponse(
        player_load_track=PlayerLoadTrackResponse()
    )

    stop_ret = authed_entry.send_command(
        ExtensionCommand(player_stop=PlayerStopRequest())
    )
    assert stop_ret == ExtensionCommandResponse(player_stop=PlayerStopResponse())


def test_logout_clears_playlists(moounit: Moounit):
    tokens_struct = Struct()
    tokens_struct["access_token"] = ACCESS_TOKEN
    tokens_struct["refresh_token"] = REFRESH_TOKEN
    tokens_struct["expires_in"] = 3600
    tokens_struct["expires_at"] = 2500000000000
    tokens_struct["scope"] = "user-read-private"

    moounit.expect_command(
        MainCommand(
            get_secure=GetSecureRequest(data=PreferenceData(key="client_secret"))
        ),
        MainCommandResponse(
            get_secure=GetSecureResponse(
                data=PreferenceData(
                    key="client_secret", value=Value(string_value=CLIENT_SECRET)
                )
            )
        ),
        times=-1,
    )
    moounit.expect_command(
        MainCommand(get_secure=GetSecureRequest(data=PreferenceData(key="client_id"))),
        MainCommandResponse(
            get_secure=GetSecureResponse(
                data=PreferenceData(
                    key="client_id", value=Value(string_value=CLIENT_ID)
                )
            )
        ),
        times=-1,
    )
    moounit.expect_command(
        MainCommand(get_secure=GetSecureRequest(data=PreferenceData(key="tokens"))),
        MainCommandResponse(
            get_secure=GetSecureResponse(
                data=PreferenceData(
                    key="tokens", value=Value(struct_value=tokens_struct)
                )
            )
        ),
        times=-1,
    )
    moounit.expect_command(
        MainCommand(
            get_preference=GetPreferenceRequest(
                data=PreferenceData(key="soloist_ws_port")
            )
        ),
        MainCommandResponse(
            get_preference=GetPreferenceResponse(
                data=PreferenceData(
                    key="soloist_ws_port", value=Value(string_value="9090")
                )
            )
        ),
        times=-1,
    )
    moounit.expect_command(
        MainCommand(
            get_preference=GetPreferenceRequest(
                data=PreferenceData(key="soloist_api_key")
            )
        ),
        MainCommandResponse(
            get_preference=GetPreferenceResponse(data=PreferenceData())
        ),
        times=-1,
    )
    moounit.expect_command(
        MainCommand(
            get_secure=GetSecureRequest(data=PreferenceData(key="soloist_api_key"))
        ),
        MainCommandResponse(get_secure=GetSecureResponse(data=PreferenceData())),
        times=-1,
    )
    moounit.expect_command(
        MainCommand(
            get_preference=GetPreferenceRequest(
                data=PreferenceData(key="client_secret")
            )
        ),
        MainCommandResponse(
            get_preference=GetPreferenceResponse(data=PreferenceData())
        ),
        times=-1,
    )
    moounit.expect_command(
        MainCommand(
            get_preference=GetPreferenceRequest(data=PreferenceData(key="client_id"))
        ),
        MainCommandResponse(
            get_preference=GetPreferenceResponse(data=PreferenceData())
        ),
        times=-1,
    )
    moounit.expect_command(
        MainCommand(
            batch_http_request=BatchHttpRequest(
                requests=[
                    HttpRequest(
                        url="https://api.spotify.com/v1/me/",
                        method="GET",
                        headers={"authorization": f"Bearer {ACCESS_TOKEN}"},
                    )
                ]
            )
        ),
        MainCommandResponse(
            batch_http_request=BatchHttpResponse(
                responses=[
                    HttpResult(
                        response=HttpResponse(
                            status_code=200,
                            body=json.dumps(
                                {
                                    "id": "u1",
                                    "display_name": "Test User",
                                    "product": "free",
                                    "type": "user",
                                    "uri": "spotify:user:u1",
                                    "href": "https://api.spotify.com/v1/users/u1",
                                    "external_urls": {
                                        "spotify": "https://open.spotify.com/user/u1"
                                    },
                                }
                            ).encode("utf-8"),
                        )
                    )
                ]
            )
        ),
        times=1,
    )
    moounit.expect_command(
        MainCommand(
            set_account=SetAccountRequest(
                account=ExtensionAccountDetail(
                    id="spotify",
                    package_name="app.moosync.spotify",
                    name="Spotify",
                    bg_color="#07C330",
                    logged_in=True,
                    username="Test User",
                )
            )
        ),
        MainCommandResponse(set_account=SetAccountResponse(success=True)),
        times=1,
    )
    moounit.expect_command(
        MainCommand(
            register_user_preference=RegisterUserPreferenceRequest(
                prefs=[
                    PreferenceItem(
                        id="soloist_status",
                        pref_type=PreferenceType.PREFERENCE_TYPE_STATUS_GROUP,
                        title="Soloist Status",
                        subtitle="Current state of the Spotify Soloist playback daemon",
                    )
                ]
            )
        ),
        MainCommandResponse(register_user_preference=RegisterUserPreferenceResponse()),
        times=-1,
    )
    moounit.expect_command(
        MainCommand(
            register_user_preference=RegisterUserPreferenceRequest(
                prefs=[
                    PreferenceItem(
                        id="soloist_status",
                        pref_type=PreferenceType.PREFERENCE_TYPE_STATUS_GROUP,
                        title="Soloist Status",
                        subtitle="Current state of the Spotify Soloist playback daemon",
                    ),
                    PreferenceItem(
                        pref_type=PreferenceType.PREFERENCE_TYPE_TEXT_INPUT_GROUP,
                        title="Client ID",
                        id="client_id",
                        subtitle="Client ID for Spotify",
                    ),
                    PreferenceItem(
                        pref_type=PreferenceType.PREFERENCE_TYPE_TEXT_INPUT_GROUP,
                        title="Client Secret",
                        id="client_secret",
                        subtitle="Client Secret for Spotify",
                    ),
                    PreferenceItem(
                        pref_type=PreferenceType.PREFERENCE_TYPE_TEXT_INPUT_GROUP,
                        title="Soloist API Key",
                        id="soloist_api_key",
                        subtitle="Spotify Soloist API Key for playback",
                    ),
                    PreferenceItem(
                        pref_type=PreferenceType.PREFERENCE_TYPE_TEXT_INPUT_GROUP,
                        title="Soloist WebSocket Port",
                        id="soloist_ws_port",
                        subtitle="WebSocket port for Spotify Soloist (default: 9090)",
                        value=PreferenceValue(string_value="9090"),
                        default=PreferenceValue(string_value="9090"),
                    ),
                ]
            )
        ),
        MainCommandResponse(register_user_preference=RegisterUserPreferenceResponse()),
        times=-1,
    )
    moounit.expect_command(
        MainCommand(register_oauth=RegisterOauthRequest(url="moosync.app")),
        MainCommandResponse(register_oauth=RegisterOauthResponse(success=True)),
        times=-1,
    )
    moounit.expect_command(
        MainCommand(register_oauth=RegisterOauthRequest(url="app.moosync.spotify")),
        MainCommandResponse(register_oauth=RegisterOauthResponse(success=True)),
        times=-1,
    )
    moounit.replay_ignore(
        MainCommand(),
        ignore_fields=["register_user_preference.prefs.value"],
    )
    moounit.call_entry()

    moounit.expect_command(
        MainCommand(set_secure=SetSecureRequest(data=PreferenceData(key="tokens"))),
        MainCommandResponse(set_secure=SetSecureResponse(success=True)),
        times=1,
    )
    moounit.expect_command(
        MainCommand(
            set_account=SetAccountRequest(
                account=ExtensionAccountDetail(
                    id="spotify",
                    package_name="app.moosync.spotify",
                    name="Spotify",
                    bg_color="#07C330",
                    logged_in=False,
                )
            )
        ),
        MainCommandResponse(set_account=SetAccountResponse(success=True)),
        times=1,
    )

    logout_ret = moounit.send_command(
        ExtensionCommand(
            perform_account_login=PerformAccountLoginRequest(login_status=False)
        )
    )
    assert logout_ret.HasField("perform_account_login")

    with pytest.raises(Exception) as exc_info:
        moounit.send_command(
            ExtensionCommand(requested_playlists=RequestedPlaylistsRequest())
        )
    assert "Not logged in" in str(exc_info.value)
