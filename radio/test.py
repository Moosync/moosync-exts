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
from core.types.protos.songs_pb2 import InnerSong, SearchResult, Song
from moounit import Moounit


@pytest.fixture
def entry(moounit: Moounit):
    try:
        moounit.call_entry()
    except Exception as e:
        print(f"Failed to call entry: {e}")
        raise e
    return moounit


def test_get_provider_scopes(entry: Moounit):
    scopes = sorted(s.scope for s in entry.manifest.scopes)

    assert len(scopes) == 1
    assert scopes == [ExtensionProviderScope.SEARCH]


def test_search(entry: Moounit):
    ret = entry.send_command(
        ExtensionCommand(
            requested_search_result=RequestedSearchResultRequest(query="pop")
        )
    )

    print(ret)
    assert len(ret.requested_search_result.songs) > 0


def test_search_cache_hit(entry: Moounit):
    cached_search = SearchResult(
        songs=[
            Song(
                song=InnerSong(
                    id="radio:cached_radio_1",
                    title="Cached Radio Station",
                )
            )
        ]
    )
    entry.expect_command(
        command=MainCommand(get_cache=GetCacheRequest(key="search:chill")),
        response=MainCommandResponse(
            get_cache=GetCacheResponse(data=cached_search.SerializeToString())
        ),
    )

    ret = entry.send_command(
        ExtensionCommand(
            requested_search_result=RequestedSearchResultRequest(query="chill")
        )
    )

    assert ret.HasField("requested_search_result")
    assert len(ret.requested_search_result.songs) == 1
    assert ret.requested_search_result.songs[0].song.title == "Cached Radio Station"
