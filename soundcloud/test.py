# pylint: disable=no-member,unused-import
import pytest
import core.types.protos.extensions_pb2 as pb
from moounit import Moounit


@pytest.fixture
def entry(moounit: Moounit):
    moounit.call_entry()
    return moounit


def test_get_provider_scopes(entry: Moounit):
    scopes = list(entry.manifest.scopes)
    scopes.sort()

    assert len(scopes) == 3
    expected = [
        pb.ExtensionProviderScope.SEARCH,
        pb.ExtensionProviderScope.PLAYBACK_DETAILS,
        pb.ExtensionProviderScope.PLAYLIST_SONGS,
    ]
    expected.sort()
    assert scopes == expected


def test_search(entry: Moounit):
    entry.expect_system_time(return_value=1234567890, times=-1)
    ret = entry.send_command(
        pb.ExtensionCommand(
            requested_search_result=pb.RequestedSearchResultRequest(query="test")
        )
    )

    assert len(ret.requested_search_result.songs) > 0
