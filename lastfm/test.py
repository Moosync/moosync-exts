# pylint: disable=no-member,unused-import
import pytest
from core.types.protos.extensions_pb2 import ExtensionProviderScope
from core.types.protos.extensions_pb2 import ExtensionCommand
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

    # Expect sync_account
    moounit.expect_command(
        MainCommand(
            set_account=pb.SetAccountRequest(
                account=pb.ExtensionAccountDetail(
                    id="lastfm",
                    package_name="lastfm",
                    name="Last.fm",
                    bg_color="red",
                    icon="",
                    logged_in=False,
                )
            )
        ),
        MainCommandResponse(set_account=pb.SetAccountResponse(success=True)),
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
    scopes = sorted(s.scope for s in entry.manifest.scopes)

    assert len(scopes) == 3
    assert scopes == sorted(
        [
            ExtensionProviderScope.SCROBBLE,
            ExtensionProviderScope.ACCOUNTS,
            ExtensionProviderScope.PLAYER_DATA_EVENTS,
        ]
    )


def test_login(entry: Moounit):
    url = (
        "http://www.last.fm/api/auth/?"
        "api_key=85ac9a982f718673f1cdbb9d8d6fba19&cb=https://moosync.app/lastfm"
    )
    entry.expect_command(
        MainCommand(open_external_url=pb.OpenExternalUrlRequest(url=url)),
        MainCommandResponse(open_external_url=pb.OpenExternalUrlResponse(success=True)),
    )
    ret = entry.send_command(
        ExtensionCommand(
            perform_account_login=pb.PerformAccountLoginRequest(
                account_id="lastfm",
                login_status=True,
            )
        )
    )
    assert ret.perform_account_login.url == url
