# pylint: disable=no-member,unused-import
import pytest
from core.types.protos.extensions_pb2 import ExtensionProviderScope
from core.types.protos.extensions_pb2 import GetProviderScopesRequest
from core.types.protos.extensions_pb2 import ExtensionCommand
from core.types.protos.extensions_pb2 import GetAccountsRequest
from core.types.protos.extensions_pb2 import MainCommand
from core.types.protos.extensions_pb2 import MainCommandResponse
import core.types.protos.extensions_pb2 as pb
from moounit import Moounit


@pytest.fixture
def entry(moounit: Moounit):
    # Expect fetch_session
    moounit.expect_command(
        MainCommand(
            get_secure=pb.GetSecureRequest(data=pb.PreferenceData(key="session"))
        ),
        MainCommandResponse(get_secure=pb.GetSecureResponse(data=pb.PreferenceData())),
    )

    # Expect register_oauth calls
    moounit.expect_command(
        MainCommand(register_oauth=pb.RegisterOauthRequest(url="lastfmcallback")),
        MainCommandResponse(register_oauth=pb.RegisterOauthResponse(success=True)),
    )

    moounit.expect_command(
        MainCommand(register_oauth=pb.RegisterOauthRequest(url="lastfm")),
        MainCommandResponse(register_oauth=pb.RegisterOauthResponse(success=True)),
    )

    moounit.call_entry()
    return moounit


def test_get_provider_scopes(entry: Moounit):
    ret = entry.send_command(
        ExtensionCommand(get_provider_scopes=GetProviderScopesRequest())
    )

    scopes = ret.get_provider_scopes.scopes
    scopes.sort()

    assert len(scopes) == 2
    assert scopes == [
        ExtensionProviderScope.SCROBBLE,
        ExtensionProviderScope.ACCOUNTS,
    ]


def test_get_accounts(entry: Moounit):
    ret = entry.send_command(ExtensionCommand(get_accounts=GetAccountsRequest()))

    assert len(ret.get_accounts.accounts) == 1
    assert ret.get_accounts.accounts[0].id == "lastfm"
    assert ret.get_accounts.accounts[0].name == "Last.fm"
    assert ret.get_accounts.accounts[0].logged_in is False
