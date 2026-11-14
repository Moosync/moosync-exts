# pylint: disable=no-member,unused-import
import pytest
import core.types.protos.extensions_pb2 as pb
from core.types.protos.preferences_pb2 import PreferenceItem, PreferenceType
from core.types.protos.songs_pb2 import Album, Artist, InnerSong, SearchResult, Song
from moounit import Moounit


@pytest.fixture
def entry(moounit: Moounit):
    moounit.expect_command(
        pb.MainCommand(
            register_user_preference=pb.RegisterUserPreferenceRequest(
                prefs=[
                    PreferenceItem(
                        pref_type=PreferenceType.PREFERENCE_TYPE_TEXT_INPUT_GROUP,
                        title="Client ID",
                        id="client_id",
                        subtitle="Client ID for Youtube",
                    ),
                    PreferenceItem(
                        pref_type=PreferenceType.PREFERENCE_TYPE_TEXT_INPUT_GROUP,
                        title="Client Secret",
                        id="client_secret",
                        subtitle="Client Secret for Youtube",
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
    scopes = list(entry.manifest.scopes)
    scopes.sort()

    assert len(scopes) == 7
    expected = [
        pb.ExtensionProviderScope.PLAYLISTS,
        pb.ExtensionProviderScope.PLAYLIST_SONGS,
        pb.ExtensionProviderScope.ACCOUNTS,
        pb.ExtensionProviderScope.SEARCH,
        pb.ExtensionProviderScope.ALBUM_SONGS,
        pb.ExtensionProviderScope.ARTIST_SONGS,
        pb.ExtensionProviderScope.RECOMMENDATIONS,
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


def test_search_cache_hit(entry: Moounit):
    cached_search = SearchResult(
        songs=[
            Song(
                song=InnerSong(
                    id="youtube:cached_yt_1",
                    title="Cached YouTube Video",
                )
            )
        ]
    )
    entry.expect_command(
        command=pb.MainCommand(get_cache=pb.GetCacheRequest(key="search:synthwave")),
        response=pb.MainCommandResponse(
            get_cache=pb.GetCacheResponse(data=cached_search.SerializeToString())
        ),
    )

    ret = entry.send_command(
        pb.ExtensionCommand(
            requested_search_result=pb.RequestedSearchResultRequest(query="synthwave")
        )
    )

    assert ret.HasField("requested_search_result")
    assert len(ret.requested_search_result.songs) == 1
    assert ret.requested_search_result.songs[0].song.title == "Cached YouTube Video"
