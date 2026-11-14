from core.types.protos.extensions_pb2 import ExtensionProviderScope
from moounit import Moounit
import pytest


@pytest.fixture
def entry(moounit: Moounit):
    moounit.call_entry()
    return moounit


def test_entry(entry: Moounit):
    scopes = list(entry.manifest.scopes)
    scopes.sort()

    assert len(scopes) == 5
    assert scopes == [
        ExtensionProviderScope.SEARCH,
        ExtensionProviderScope.PLAYLIST_SONGS,
        ExtensionProviderScope.ARTIST_SONGS,
        ExtensionProviderScope.ALBUM_SONGS,
        ExtensionProviderScope.PLAYBACK_DETAILS,
    ]
