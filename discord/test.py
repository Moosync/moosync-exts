# pylint: disable=no-member
import core.types.protos.extensions_pb2 as pb
import pytest
from moounit import Moounit


@pytest.fixture
def entry(moounit: Moounit):
    moounit.call_entry()
    return moounit


def test_get_provider_scopes(entry: Moounit):
    ret = entry.send_command(
        pb.ExtensionCommand(get_provider_scopes=pb.GetProviderScopesRequest())
    )

    scopes = ret.get_provider_scopes.scopes
    scopes.sort()

    assert len(scopes) == 2
    assert scopes == [
        pb.ExtensionProviderScope.PLAYER_UI_EVENTS,
        pb.ExtensionProviderScope.PLAYER_DATA_EVENTS,
    ]
