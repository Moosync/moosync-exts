# pylint: disable=no-member,unused-import
import pytest
from core.types.protos.extensions_pb2 import (
    ExtensionCommand,
    ExtensionProviderScope,
    GetAccountsRequest,
    GetCacheRequest,
    GetCacheResponse,
    GetProviderScopesRequest,
    GetSecureRequest,
    GetSecureResponse,
    MainCommand,
    MainCommandResponse,
    PreferenceData,
    RegisterUserPreferenceRequest,
    RegisterUserPreferenceResponse,
    RequestedSearchResultRequest,
)
from core.types.protos.songs_pb2 import InnerSong, SearchResult, Song
import core.types.protos.ui_pb2 as ui_pb
from moounit import Moounit


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
            register_user_preference=RegisterUserPreferenceRequest(
                prefs=[
                    ui_pb.PreferenceUiData(
                        type=ui_pb.PreferenceTypes.EDIT_TEXT,
                        title="Client ID",
                        key="client_id",
                        description="Client ID for Spotify",
                        mobile=True,
                    ),
                    ui_pb.PreferenceUiData(
                        type=ui_pb.PreferenceTypes.EDIT_TEXT,
                        title="Client Secret",
                        key="client_secret",
                        description="Client Secret for Spotify",
                        mobile=True,
                    ),
                ]
            )
        ),
        MainCommandResponse(register_user_preference=RegisterUserPreferenceResponse()),
    )
    try:
        moounit.call_entry()
    except Exception as e:
        print(f"Failed to call entry: {e}")
        # Re-raise to fail the test and see logs
        raise e
    return moounit


def test_get_provider_scopes(entry: Moounit):
    scopes = list(entry.manifest.scopes)
    assert len(scopes) == 0


def test_get_accounts(entry: Moounit):
    ret = entry.send_command(ExtensionCommand(get_accounts=GetAccountsRequest()))

    # The field in response is likely 'get_accounts' too
    assert len(ret.get_accounts.accounts) == 1
    assert ret.get_accounts.accounts[0].id == "spotify"
    assert ret.get_accounts.accounts[0].name == "Spotify"
    assert ret.get_accounts.accounts[0].logged_in is False


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
