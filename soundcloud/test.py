# pylint: disable=no-member,unused-import
import pytest
import core.types.protos.extensions_pb2 as pb
from core.types.protos.songs_pb2 import InnerSong, SearchResult, Song
from moounit import Moounit


@pytest.fixture
def entry(moounit: Moounit):
    moounit.call_entry()
    return moounit


def test_get_provider_scopes(entry: Moounit):
    scopes = sorted(s.scope for s in entry.manifest.scopes)

    assert len(scopes) == 3
    expected = sorted(
        [
            pb.ExtensionProviderScope.SEARCH,
            pb.ExtensionProviderScope.PLAYBACK_DETAILS,
            pb.ExtensionProviderScope.PLAYLIST_SONGS,
        ]
    )
    assert scopes == expected


def test_search(entry: Moounit):
    ret = entry.send_command(
        pb.ExtensionCommand(
            requested_search_result=pb.RequestedSearchResultRequest(query="test")
        )
    )

    assert len(ret.requested_search_result.songs) > 0
    assert ret.requested_search_result.songs[0].song.id.startswith(
        "moosync.soundcloud:"
    )
    assert not ret.requested_search_result.songs[0].song.HasField("playback_url")


def test_playback_details(entry: Moounit):
    ret = entry.send_command(
        pb.ExtensionCommand(
            playback_details_requested=pb.PlaybackDetailsRequestedRequest(
                song=Song(
                    song=InnerSong(
                        id="moosync.soundcloud:2428757886",
                    )
                )
            )
        )
    )

    assert ret.HasField("playback_details_requested")
    assert ret.playback_details_requested.url.startswith("https://")
    assert ret.playback_details_requested.duration.seconds == 180
    assert ret.playback_details_requested.HasField("expires_at")
    assert ret.playback_details_requested.expires_at.seconds > 0


def test_search_cache_hit(entry: Moounit):
    cached_search = SearchResult(
        songs=[
            Song(
                song=InnerSong(
                    id="soundcloud:cached_sc_1",
                    title="Cached SoundCloud Track",
                )
            )
        ]
    )
    entry.expect_command(
        command=pb.MainCommand(get_cache=pb.GetCacheRequest(key="search:electronic")),
        response=pb.MainCommandResponse(
            get_cache=pb.GetCacheResponse(data=cached_search.SerializeToString())
        ),
    )

    ret = entry.send_command(
        pb.ExtensionCommand(
            requested_search_result=pb.RequestedSearchResultRequest(query="electronic")
        )
    )

    assert ret.HasField("requested_search_result")
    assert len(ret.requested_search_result.songs) == 1
    assert ret.requested_search_result.songs[0].song.title == "Cached SoundCloud Track"
