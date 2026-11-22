# pylint: disable=no-member,unused-import
import core.types.protos.extensions_pb2 as pb
from core.types.protos.extensions_pb2 import (
    BatchHttpRequest,
    CustomRequest,
    ExtensionCommand,
    ExtensionProviderScope,
    GetCacheRequest,
    GetCacheResponse,
    MainCommand,
    MainCommandResponse,
    PlaybackDetailsRequestedRequest,
    RequestedAlbumSongsRequest,
    RequestedArtistSongsRequest,
    RequestedPlaylistSongsRequest,
    RequestedPlaylistsRequest,
    RequestedSearchResultRequest,
)
from core.types.protos.preferences_pb2 import PreferenceItem, PreferenceType
from core.types.protos.songs_pb2 import (
    Album,
    Artist,
    InnerSong,
    PlaybackUrl,
    SearchResult,
    Song,
)
from google.protobuf.struct_pb2 import Value
import pytest
from moounit import Moounit


@pytest.fixture
def entry(moounit: Moounit):
    moounit.expect_command(
        MainCommand(
            get_preference=pb.GetPreferenceRequest(
                data=pb.PreferenceData(key="koel_instance_url")
            )
        ),
        MainCommandResponse(
            get_preference=pb.GetPreferenceResponse(data=pb.PreferenceData())
        ),
        times=-1,
    )
    moounit.expect_command(
        MainCommand(
            get_preference=pb.GetPreferenceRequest(
                data=pb.PreferenceData(key="koel_username")
            )
        ),
        MainCommandResponse(
            get_preference=pb.GetPreferenceResponse(data=pb.PreferenceData())
        ),
    )
    moounit.expect_command(
        MainCommand(
            get_secure=pb.GetSecureRequest(data=pb.PreferenceData(key="koel_password"))
        ),
        MainCommandResponse(get_secure=pb.GetSecureResponse(data=pb.PreferenceData())),
    )
    moounit.expect_command(
        MainCommand(
            get_preference=pb.GetPreferenceRequest(
                data=pb.PreferenceData(key="koel_password")
            )
        ),
        MainCommandResponse(
            get_preference=pb.GetPreferenceResponse(data=pb.PreferenceData())
        ),
    )
    moounit.expect_command(
        MainCommand(
            register_user_preference=pb.RegisterUserPreferenceRequest(
                prefs=[
                    PreferenceItem(
                        pref_type=PreferenceType.PREFERENCE_TYPE_TEXT_INPUT_GROUP,
                        title="Instance URL",
                        id="koel_instance_url",
                        subtitle="Full URL to your koel instance",
                    ),
                    PreferenceItem(
                        pref_type=PreferenceType.PREFERENCE_TYPE_TEXT_INPUT_GROUP,
                        title="Email",
                        id="koel_username",
                        subtitle="Email for your koel account",
                    ),
                    PreferenceItem(
                        pref_type=PreferenceType.PREFERENCE_TYPE_TEXT_INPUT_GROUP,
                        title="Password",
                        id="koel_password",
                        subtitle="Password for your koel account",
                    ),
                ]
            )
        ),
        MainCommandResponse(
            register_user_preference=pb.RegisterUserPreferenceResponse()
        ),
    )
    moounit.call_entry()
    return moounit


@pytest.fixture
def logged_in_entry(moounit: Moounit):
    moounit.replay_ignore(
        request=MainCommand(),
        ignore_fields=["batch_http_request.requests.headers.Authorization"],
    )
    moounit.replay_ignore(
        request=BatchHttpRequest(),
        ignore_fields=["headers.Authorization"],
    )
    moounit.expect_command(
        MainCommand(
            get_preference=pb.GetPreferenceRequest(
                data=pb.PreferenceData(key="koel_instance_url")
            )
        ),
        MainCommandResponse(
            get_preference=pb.GetPreferenceResponse(
                data=pb.PreferenceData(
                    key="koel_instance_url",
                    value=Value(string_value="http://localhost"),
                )
            )
        ),
        times=-1,
    )
    moounit.expect_command(
        MainCommand(
            get_preference=pb.GetPreferenceRequest(
                data=pb.PreferenceData(key="koel_username")
            )
        ),
        MainCommandResponse(
            get_preference=pb.GetPreferenceResponse(
                data=pb.PreferenceData(
                    key="koel_username",
                    value=Value(string_value="admin@koel.dev"),
                )
            )
        ),
    )
    moounit.expect_command(
        MainCommand(
            get_secure=pb.GetSecureRequest(data=pb.PreferenceData(key="koel_password"))
        ),
        MainCommandResponse(
            get_secure=pb.GetSecureResponse(
                data=pb.PreferenceData(
                    key="koel_password",
                    value=Value(string_value="KoelIsCool"),
                )
            )
        ),
    )
    moounit.expect_command(
        MainCommand(
            register_user_preference=pb.RegisterUserPreferenceRequest(
                prefs=[
                    PreferenceItem(
                        pref_type=PreferenceType.PREFERENCE_TYPE_TEXT_INPUT_GROUP,
                        title="Instance URL",
                        id="koel_instance_url",
                        subtitle="Full URL to your koel instance",
                    ),
                    PreferenceItem(
                        pref_type=PreferenceType.PREFERENCE_TYPE_TEXT_INPUT_GROUP,
                        title="Email",
                        id="koel_username",
                        subtitle="Email for your koel account",
                    ),
                    PreferenceItem(
                        pref_type=PreferenceType.PREFERENCE_TYPE_TEXT_INPUT_GROUP,
                        title="Password",
                        id="koel_password",
                        subtitle="Password for your koel account",
                    ),
                ]
            )
        ),
        MainCommandResponse(
            register_user_preference=pb.RegisterUserPreferenceResponse()
        ),
    )
    moounit.call_entry()
    return moounit


def test_get_provider_scopes(entry: Moounit):
    scopes = sorted(s.scope for s in entry.manifest.scopes)

    assert len(scopes) == 7
    expected_scopes = sorted(
        [
            pb.ExtensionProviderScope.SEARCH,
            pb.ExtensionProviderScope.PLAYLIST_SONGS,
            pb.ExtensionProviderScope.ARTIST_SONGS,
            pb.ExtensionProviderScope.ALBUM_SONGS,
            pb.ExtensionProviderScope.ACCOUNTS,
            pb.ExtensionProviderScope.PLAYLISTS,
            pb.ExtensionProviderScope.PLAYBACK_DETAILS,
        ]
    )
    assert scopes == expected_scopes


def test_search(logged_in_entry: Moounit):
    ret = logged_in_entry.send_command(
        ExtensionCommand(
            requested_search_result=RequestedSearchResultRequest(query="Moosync")
        )
    )
    assert ret.HasField("requested_search_result")
    songs = ret.requested_search_result.songs
    assert len(songs) == 1
    assert songs[0].song.id == "moosync.koel:01a1fced-ebca-719a-b304-867b76bf0b3f"
    assert songs[0].song.title == "Moosync Test Track"


def test_requested_playlists(logged_in_entry: Moounit):
    ret = logged_in_entry.send_command(
        ExtensionCommand(requested_playlists=RequestedPlaylistsRequest())
    )
    assert ret.HasField("requested_playlists")
    playlists = ret.requested_playlists.playlists
    assert len(playlists) == 3
    names = [p.playlist_name for p in playlists]
    assert "Moosync Test Playlist" in names


def test_playlist_songs(logged_in_entry: Moounit):
    ret = logged_in_entry.send_command(
        ExtensionCommand(
            requested_playlist_songs=RequestedPlaylistSongsRequest(
                id="01a1fcee-5964-73c3-a609-da60199fc49c"
            )
        )
    )
    assert ret.HasField("requested_playlist_songs")
    songs = ret.requested_playlist_songs.songs
    assert len(songs) == 1
    assert songs[0].song.id == "moosync.koel:01a1fced-ebca-719a-b304-867b76bf0b3f"
    assert songs[0].song.title == "Moosync Test Track"


def test_album_songs(logged_in_entry: Moounit):
    ret = logged_in_entry.send_command(
        ExtensionCommand(
            requested_album_songs=RequestedAlbumSongsRequest(
                album=Album(album_name="Test Album")
            )
        )
    )
    assert ret.HasField("requested_album_songs")
    songs = ret.requested_album_songs.songs
    assert len(songs) == 1
    assert songs[0].song.id == "moosync.koel:01a1fced-ebca-719a-b304-867b76bf0b3f"
    assert songs[0].song.title == "Moosync Test Track"


def test_artist_songs(logged_in_entry: Moounit):
    ret = logged_in_entry.send_command(
        ExtensionCommand(
            requested_artist_songs=RequestedArtistSongsRequest(
                artist=Artist(artist_name="Test Artist")
            )
        )
    )
    assert ret.HasField("requested_artist_songs")
    songs = ret.requested_artist_songs.songs
    assert len(songs) == 1
    assert songs[0].song.id == "moosync.koel:01a1fced-ebca-719a-b304-867b76bf0b3f"
    assert songs[0].song.title == "Moosync Test Track"


def test_custom_request(logged_in_entry: Moounit):
    ret = logged_in_entry.send_command(
        ExtensionCommand(
            custom_request=CustomRequest(
                request_id="extension://moosync.koel/01a1fced-ebca-719a-b304-867b76bf0b3f"
            )
        )
    )
    assert ret.HasField("custom_request")
    assert ret.custom_request.redirect_url.startswith(
        "http://localhost/play/01a1fced-ebca-719a-b304-867b76bf0b3f?t="
    )


def test_playback_details_requested(logged_in_entry: Moounit):
    song_id = "01a1fced-ebca-719a-b304-867b76bf0b3f"
    ret = logged_in_entry.send_command(
        ExtensionCommand(
            playback_details_requested=PlaybackDetailsRequestedRequest(
                song=Song(
                    song=InnerSong(
                        id=f"moosync.koel:{song_id}",
                        playback_url=PlaybackUrl(
                            url=f"extension://moosync.koel/{song_id}"
                        ),
                    )
                )
            )
        )
    )
    assert ret.HasField("playback_details_requested")
    assert ret.playback_details_requested.url.startswith(
        f"http://localhost/play/{song_id}?t="
    )


def test_search_cache_hit(entry: Moounit):
    cached_search = SearchResult(
        songs=[
            Song(
                song=InnerSong(
                    id="koel:cached_koel_1",
                    title="Cached Koel Song",
                )
            )
        ]
    )
    entry.expect_command(
        command=MainCommand(get_cache=GetCacheRequest(key="search:ambient")),
        response=MainCommandResponse(
            get_cache=GetCacheResponse(data=cached_search.SerializeToString())
        ),
    )

    ret = entry.send_command(
        ExtensionCommand(
            requested_search_result=RequestedSearchResultRequest(query="ambient")
        )
    )

    assert ret.HasField("requested_search_result")
    assert len(ret.requested_search_result.songs) == 1
    assert ret.requested_search_result.songs[0].song.title == "Cached Koel Song"
