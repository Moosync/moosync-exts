# pylint: disable=no-member,unused-import
import pytest
import core.types.protos.extensions_pb2 as pb
import core.types.protos.ui_pb2
from core.types.protos.songs_pb2 import Album
from core.types.protos.songs_pb2 import Artist
from moounit import Moounit


@pytest.fixture
def entry(moounit: Moounit):
    moounit.expect_command(
        pb.MainCommand(
            register_user_preference=pb.RegisterUserPreferenceRequest(
                prefs=[
                    core.types.protos.ui_pb2.PreferenceUiData(
                        type=core.types.protos.ui_pb2.PreferenceTypes.EDIT_TEXT,
                        title="Client ID",
                        key="client_id",
                        description="Client ID for Youtube",
                        mobile=True,
                    ),
                    core.types.protos.ui_pb2.PreferenceUiData(
                        type=core.types.protos.ui_pb2.PreferenceTypes.EDIT_TEXT,
                        title="Client Secret",
                        key="client_secret",
                        description="Client Secret for Youtube",
                        mobile=True,
                    ),
                ]
            )
        ),
        pb.MainCommandResponse(
            register_user_preference=pb.RegisterUserPreferenceResponse()
        ),
    )
    moounit.expect_command(
        pb.MainCommand(
            get_preference=pb.GetPreferenceRequest(
                data=pb.PreferenceData(key="client_id")
            )
        ),
        pb.MainCommandResponse(
            get_preference=pb.GetPreferenceResponse(data=pb.PreferenceData())
        ),
    )
    # The new() method of YoutubeExtension also calls get_preference("client_secret")
    # if "client_id" was found, but let's assume we return empty for client_id first,
    # so it might skip client_secret or not.
    # Looking at the code: it checks client_id result. If OK (even if empty value?),
    # it checks client_secret.
    # The mock returns empty PreferenceData by default if we don't specify value.
    # So we should probably expect client_secret too.
    moounit.expect_command(
        pb.MainCommand(
            get_preference=pb.GetPreferenceRequest(
                data=pb.PreferenceData(key="client_secret")
            )
        ),
        pb.MainCommandResponse(
            get_preference=pb.GetPreferenceResponse(data=pb.PreferenceData())
        ),
    )
    moounit.call_entry()
    return moounit


def test_get_provider_scopes(entry: Moounit):
    ret = entry.send_command(
        pb.ExtensionCommand(get_provider_scopes=pb.GetProviderScopesRequest())
    )

    scopes = ret.get_provider_scopes.scopes
    scopes.sort()

    assert len(scopes) == 3
    expected = [
        pb.ExtensionProviderScope.PLAYLISTS,
        pb.ExtensionProviderScope.PLAYLIST_SONGS,
        pb.ExtensionProviderScope.ACCOUNTS,
    ]
    expected.sort()
    assert scopes == expected


def test_search(entry: Moounit):
    ret = entry.send_command(
        pb.ExtensionCommand(
            requested_search_result=pb.RequestedSearchResultRequest(query="test")
        )
    )

    assert len(ret.requested_search_result.songs) > 0


def test_get_album_songs(entry: Moounit):
    ret = entry.send_command(
        pb.ExtensionCommand(
            requested_album_songs=pb.RequestedAlbumSongsRequest(
                album=Album(album_name="test album")
            )
        )
    )

    assert len(ret.requested_album_songs.songs) > 0


def test_get_artist_songs(entry: Moounit):
    ret = entry.send_command(
        pb.ExtensionCommand(
            requested_artist_songs=pb.RequestedArtistSongsRequest(
                artist=Artist(artist_name="test artist")
            )
        )
    )

    assert len(ret.requested_artist_songs.songs) > 0


def test_get_recommendations(entry: Moounit):
    ret = entry.send_command(
        pb.ExtensionCommand(
            requested_recommendations=pb.RequestedRecommendationsRequest()
        )
    )

    assert len(ret.requested_recommendations.songs) > 0
