# pylint: disable=no-member,unused-import
import pytest
from core.types.protos.extensions_pb2 import ExtensionProviderScope
from core.types.protos.extensions_pb2 import GetProviderScopesRequest
from core.types.protos.extensions_pb2 import ExtensionCommand
from core.types.protos.extensions_pb2 import GetAccountsRequest
from core.types.protos.extensions_pb2 import MainCommand
from core.types.protos.extensions_pb2 import MainCommandResponse
from core.types.protos.extensions_pb2 import RegisterUserPreferenceResponse
from core.types.protos.extensions_pb2 import RegisterUserPreferenceRequest
from core.types.protos.extensions_pb2 import PreferenceData
from core.types.protos.extensions_pb2 import GetSecureRequest
from core.types.protos.extensions_pb2 import GetSecureResponse
import core.types.protos.ui_pb2 as ui_pb
from moounit import Moounit


@pytest.fixture
def entry(moounit: Moounit):
    moounit.expect_command(
        MainCommand(
            get_secure=GetSecureRequest(data=PreferenceData(key="client_secret"))
        ),
        MainCommandResponse(get_secure=GetSecureResponse(data=PreferenceData())),
    )
    moounit.expect_command(
        MainCommand(get_secure=GetSecureRequest(data=PreferenceData(key="client_id"))),
        MainCommandResponse(get_secure=GetSecureResponse(data=PreferenceData())),
    )
    moounit.expect_command(
        MainCommand(get_secure=GetSecureRequest(data=PreferenceData(key="tokens"))),
        MainCommandResponse(get_secure=GetSecureResponse(data=PreferenceData())),
    )
    moounit.expect_command(
        MainCommand(
            register_user_preference=RegisterUserPreferenceRequest(
                prefs=[
                    ui_pb.PreferenceUiData(
                        type=ui_pb.PreferenceTypes.EDIT_TEXT,
                        title="Client ID",
                        key="client_id",
                        description="Client ID for Spotify",
                        mobile=True,
                    ),
                    ui_pb.PreferenceUiData(
                        type=ui_pb.PreferenceTypes.EDIT_TEXT,
                        title="Client Secret",
                        key="client_secret",
                        description="Client Secret for Spotify",
                        mobile=True,
                    ),
                ]
            )
        ),
        MainCommandResponse(register_user_preference=RegisterUserPreferenceResponse()),
    )
    try:
        moounit.call_entry()
    except Exception as e:
        print(f"Failed to call entry: {e}")
        # Re-raise to fail the test and see logs
        raise e
    return moounit


def test_get_provider_scopes(entry: Moounit):
    ret = entry.send_command(
        ExtensionCommand(get_provider_scopes=GetProviderScopesRequest())
    )

    scopes = ret.get_provider_scopes.scopes
    # Spotify has no scopes if not logged in
    assert len(scopes) == 0


def test_get_accounts(entry: Moounit):
    ret = entry.send_command(ExtensionCommand(get_accounts=GetAccountsRequest()))

    # The field in response is likely 'get_accounts' too
    assert len(ret.get_accounts.accounts) == 1
    assert ret.get_accounts.accounts[0].id == "spotify"
    assert ret.get_accounts.accounts[0].name == "Spotify"
    assert ret.get_accounts.accounts[0].logged_in is False
