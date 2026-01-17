import pytest
from core.types.protos.extensions_pb2 import ExtensionProviderScope
from core.types.protos.extensions_pb2 import GetProviderScopesRequest
from core.types.protos.extensions_pb2 import ExtensionCommand
from core.types.protos.extensions_pb2 import RequestedSearchResultRequest
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
    ret = entry.send_command(
        ExtensionCommand(get_provider_scopes=GetProviderScopesRequest())
    )

    scopes = ret.get_provider_scopes.scopes
    scopes.sort()

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
