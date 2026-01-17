# pylint: disable=no-member,unused-import
import os
import struct
import json
import pytest
import core.types.protos.extensions_pb2 as pb
from moounit import Moounit


@pytest.fixture
def entry(moounit: Moounit):
    moounit.expect_open_clientfd("/discord-ipc-0", return_value=375)
    handshake_payload = b'{"v":1,"client_id":"867757838679670784"}'
    handshake_header = struct.pack("<II", 0, len(handshake_payload))
    moounit.expect_write_sock(
        375,
        bytearray(handshake_header + handshake_payload),
        times=1,
        return_value=len(handshake_header + handshake_payload),
    )

    resp_payload = json.dumps(
        {"cmd": "DISPATCH", "data": {}, "evt": "READY", "nonce": None}
    ).encode("utf-8")
    resp_header = struct.pack("<II", 1, len(resp_payload))

    moounit.expect_read_sock(375, 8, return_value=resp_header)
    moounit.expect_read_sock(375, len(resp_payload), return_value=resp_payload)
    activity_data = {
        "state": " - Misc",
        "details": " (Paused)",
        "timestamps": {"start": 0},
        "assets": {
            "large_image": "logo_border",
            "large_text": "Title",
            "small_image": "logo_circle",
            "small_text": "Moosync",
        },
        "instance": True,
    }

    activity_req = {
        "cmd": "SET_ACTIVITY",
        "args": {
            "activity": activity_data,
            "pid": os.getpid(),
        },
        "nonce": "00000000-0000-0000-0000-000000000000",
    }
    activity_payload = json.dumps(activity_req, separators=(",", ":")).encode("utf-8")
    activity_header = struct.pack("<II", 1, len(activity_payload))

    moounit.expect_write_sock(
        375,
        bytearray(activity_header + activity_payload),
        times=1,
        return_value=len(activity_header + activity_payload),
    )
    sa_resp_payload = json.dumps(
        {
            "cmd": "SET_ACTIVITY",
            "data": {},
            "evt": None,
            "nonce": "00000000-0000-0000-0000-000000000000",
        },
        separators=(",", ":"),
    ).encode("utf-8")
    sa_resp_header = struct.pack("<II", 1, len(sa_resp_payload))

    moounit.expect_read_sock(375, 8, return_value=sa_resp_header)
    moounit.expect_read_sock(375, len(sa_resp_payload), return_value=sa_resp_payload)
    moounit.expect_system_time(return_value=0)
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
