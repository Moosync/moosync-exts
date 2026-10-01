# pylint: disable=unused-import
import pytest
from core.types.protos.extensions_pb2 import (
    ExtensionCommand,
    ExtensionProviderScope,
    GetProviderScopesRequest,
    RequestedSearchResultRequest,
)
from moounit import Moounit
from moounit.core import BatchHttpRequest


@pytest.fixture
def entry(moounit: Moounit):
    print("calling entry")
    moounit.replay_ignore(
        request=BatchHttpRequest(),
        ignore_fields=[
            "headers.X-Goog-Visitor-Id",
            "headers.user-agent",
        ],
    )
    moounit.replay_ignore(
        request=BatchHttpRequest(),
        ignore_field=["body"],
        regex=r'"(visitorData|userAgent)":\s*"[^"]*"',
    )
    moounit.call_entry()
    return moounit


# def test_entry(entry: Moounit):
#     ret = entry.send_command(
#         ExtensionCommand(get_provider_scopes=GetProviderScopesRequest())
#     )

#     scopes = ret.get_provider_scopes.scopes
#     scopes.sort()

#     assert len(scopes) == 1
#     assert scopes == [
#         ExtensionProviderScope.SEARCH,
#     ]


def test_search(entry: Moounit):
    entry.replay_ignore(
        request=BatchHttpRequest(),
        ignore_field=[
            "headers.X-Goog-Visitor-Id",
            "body",
            "headers.user-agent",
        ],
    )
    ret = entry.send_command(
        ExtensionCommand(
            requested_search_result=RequestedSearchResultRequest(query="test")
        )
    )
    print(
        len(
            ret.requested_search_result.songs,
        ),
        len(
            ret.requested_search_result.playlists,
        ),
        len(
            ret.requested_search_result.artists,
        ),
        len(
            ret.requested_search_result.albums,
        ),
    )
