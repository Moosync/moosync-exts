# pylint: disable=no-member,unused-import
import pytest
from core.types.protos.extensions_pb2 import (
    ExtensionCommand,
    ExtensionProviderScope,
    GetCacheRequest,
    GetCacheResponse,
    MainCommand,
    MainCommandResponse,
    RequestedSearchResultRequest,
)
import core.types.protos.extensions_pb2 as pb
from core.types.protos.preferences_pb2 import PreferenceItem, PreferenceType
from core.types.protos.songs_pb2 import InnerSong, SearchResult, Song
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
    try:
        moounit.call_entry()
    except Exception as e:
        print(f"Failed to call entry: {e}")
        raise e
    return moounit


def test_get_provider_scopes(entry: Moounit):
    scopes = list(entry.manifest.scopes)
    scopes.sort()

    assert len(scopes) == 7
    expected_scopes = [
        pb.ExtensionProviderScope.SEARCH,
        pb.ExtensionProviderScope.PLAYLIST_SONGS,
        pb.ExtensionProviderScope.ARTIST_SONGS,
        pb.ExtensionProviderScope.ALBUM_SONGS,
        pb.ExtensionProviderScope.ACCOUNTS,
        pb.ExtensionProviderScope.PLAYLISTS,
        pb.ExtensionProviderScope.PLAYBACK_DETAILS,
    ]
    expected_scopes.sort()
    assert scopes == expected_scopes


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
