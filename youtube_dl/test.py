from core.types.protos.extensions_pb2 import ExtensionProviderScope
from core.types.protos.extensions_pb2 import GetProviderScopesRequest
from core.types.protos.extensions_pb2 import ExtensionCommand
from moounit import Moounit
import pytest
@pytest.fixture
def entry(moounit: Moounit):
    moounit.call_entry()
    return moounit


def test_entry(entry: Moounit):
    ret = entry.send_command(
        ExtensionCommand(get_provider_scopes=GetProviderScopesRequest())
    )

    scopes = ret.get_provider_scopes.scopes
    scopes.sort()

    assert len(scopes) == 5
    assert scopes == [
        ExtensionProviderScope.SEARCH,
        ExtensionProviderScope.PLAYLIST_SONGS,
        ExtensionProviderScope.ARTIST_SONGS,
        ExtensionProviderScope.ALBUM_SONGS,
        ExtensionProviderScope.PLAYBACK_DETAILS,
    ]
