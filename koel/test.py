# pylint: disable=no-member,unused-import
import pytest
from core.types.protos.extensions_pb2 import ExtensionProviderScope
import core.types.protos.extensions_pb2 as pb
import core.types.protos.ui_pb2 as ui_pb
from core.types.protos.extensions_pb2 import ExtensionCommand
from core.types.protos.extensions_pb2 import MainCommand
from core.types.protos.extensions_pb2 import MainCommandResponse
from moounit import Moounit


@pytest.fixture
def entry(moounit: Moounit):
    moounit.expect_command(
        MainCommand(
            get_preference=pb.GetPreferenceRequest(
                data=pb.PreferenceData(key="koel_instance_url")
            )
        ),
        MainCommandResponse(
            get_preference=pb.GetPreferenceResponse(data=pb.PreferenceData())
        ),
    )
    moounit.expect_command(
        MainCommand(
            get_preference=pb.GetPreferenceRequest(
                data=pb.PreferenceData(key="koel_username")
            )
        ),
        MainCommandResponse(
            get_preference=pb.GetPreferenceResponse(data=pb.PreferenceData())
        ),
    )
    moounit.expect_command(
        MainCommand(
            get_secure=pb.GetSecureRequest(data=pb.PreferenceData(key="koel_password"))
        ),
        MainCommandResponse(get_secure=pb.GetSecureResponse(data=pb.PreferenceData())),
    )
    moounit.expect_command(
        MainCommand(
            register_user_preference=pb.RegisterUserPreferenceRequest(
                prefs=[
                    ui_pb.PreferenceUiData(
                        type=ui_pb.PreferenceTypes.EDIT_TEXT,
                        title="Instance URL",
                        key="koel_instance_url",
                        description="Full URL to your koel instance",
                        input_type=ui_pb.InputType.TEXT,
                    ),
                    ui_pb.PreferenceUiData(
                        type=ui_pb.PreferenceTypes.EDIT_TEXT,
                        title="Email",
                        key="koel_username",
                        description="Email for your koel account",
                        input_type=ui_pb.InputType.TEXT,
                    ),
                    ui_pb.PreferenceUiData(
                        type=ui_pb.PreferenceTypes.EDIT_TEXT,
                        title="Password",
                        key="koel_password",
                        description="Password for your koel account",
                        input_type=ui_pb.InputType.TEXT,
                    ),
                ]
            )
        ),
        MainCommandResponse(
            register_user_preference=pb.RegisterUserPreferenceResponse()
        ),
    )
    try:
        moounit.call_entry()
    except Exception as e:
        print(f"Failed to call entry: {e}")
        raise e
    return moounit


def test_get_provider_scopes(entry: Moounit):
    ret = entry.send_command(
        ExtensionCommand(get_provider_scopes=pb.GetProviderScopesRequest())
    )

    scopes = ret.get_provider_scopes.scopes
    scopes.sort()

    assert len(scopes) == 8
    expected_scopes = [
        pb.ExtensionProviderScope.SEARCH,
        pb.ExtensionProviderScope.PLAYLIST_SONGS,
        pb.ExtensionProviderScope.ARTIST_SONGS,
        pb.ExtensionProviderScope.ALBUM_SONGS,
        pb.ExtensionProviderScope.ACCOUNTS,
        pb.ExtensionProviderScope.PLAYLISTS,
        pb.ExtensionProviderScope.PLAYBACK_DETAILS,
        pb.ExtensionProviderScope.SONG_FROM_URL,
    ]
    expected_scopes.sort()
    assert scopes == expected_scopes
