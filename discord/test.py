# pylint: disable=no-member,protected-access
import base64
import json
import os
import core.types.protos.extensions_pb2 as pb
import core.types.protos.songs_pb2 as songs_pb
from google.protobuf.struct_pb2 import Value
import pytest
from moounit import Moounit


@pytest.fixture
def entry(moounit: Moounit):
    moounit.replay_ignore(
        request=pb.MainCommand(write_sock=pb.WriteSockRequest()),
        ignore_fields=["write_sock.buf"],
    )
    moounit.call_entry()
    return moounit


def test_get_provider_scopes(entry: Moounit):
    scopes = sorted(s.scope for s in entry.manifest.scopes)

    assert len(scopes) == 2
    assert scopes == [
        pb.ExtensionProviderScope.PLAYER_UI_EVENTS,
        pb.ExtensionProviderScope.PLAYER_DATA_EVENTS,
    ]


def _setup_player_expectations(entry: Moounit, state: int, time_val: float):
    song = songs_pb.Song(
        song=songs_pb.InnerSong(
            id="spotify:12345",
            title="Test Track",
            type=songs_pb.SongType.SPOTIFY,
            url="4uLU6hMCjMI75M1A2tKUQC",
        ),
        artists=[songs_pb.Artist(artist_name="Test Artist")],
        album=songs_pb.Album(album_name="Test Album"),
    )
    entry.expect_command(
        pb.MainCommand(get_current_song=pb.GetCurrentSongRequest()),
        pb.MainCommandResponse(get_current_song=pb.GetCurrentSongResponse(song=song)),
    )
    entry.expect_command(
        pb.MainCommand(get_player_state=pb.GetPlayerStateRequest()),
        pb.MainCommandResponse(get_player_state=pb.GetPlayerStateResponse(state=state)),
    )
    entry.expect_command(
        pb.MainCommand(get_time=pb.GetTimeRequest()),
        pb.MainCommandResponse(get_time=pb.GetTimeResponse(time=time_val)),
    )


def test_song_changed(entry: Moounit):
    _setup_player_expectations(entry, pb.PlayerState.PLAYING, 10.0)
    resp = entry.send_command(pb.ExtensionCommand(song_changed=pb.SongChangedRequest()))
    assert resp.HasField("song_changed")


def test_player_state_changed(entry: Moounit):
    _setup_player_expectations(entry, pb.PlayerState.PAUSED, 15.0)
    resp = entry.send_command(
        pb.ExtensionCommand(
            player_state_changed=pb.PlayerStateChangedRequest(
                state=pb.PlayerState.PAUSED
            )
        )
    )
    assert resp.HasField("player_state_changed")


def test_seeked(entry: Moounit):
    _setup_player_expectations(entry, pb.PlayerState.PLAYING, 42.0)
    resp = entry.send_command(
        pb.ExtensionCommand(seeked=pb.SeekedRequest(position=42.0))
    )
    assert resp.HasField("seeked")


def test_local_song_cover_art(entry: Moounit):
    local_song = songs_pb.Song(
        song=songs_pb.InnerSong(
            id="local:123",
            title="Numb",
            type=songs_pb.SongType.LOCAL,
            song_cover_path_high="/cache/thumbnails/local.webp",
        ),
        artists=[songs_pb.Artist(artist_name="Linkin Park")],
        album=songs_pb.Album(album_name="Meteora"),
    )
    cover_url = "https://cdn-images.dzcdn.net/images/cover/meteora/500x500.jpg"
    deezer_body = json.dumps({"data": [{"album": {"cover_big": cover_url}}]}).encode(
        "utf-8"
    )

    entry.expect_command(
        pb.MainCommand(get_current_song=pb.GetCurrentSongRequest()),
        pb.MainCommandResponse(
            get_current_song=pb.GetCurrentSongResponse(song=local_song)
        ),
    )
    entry.expect_command(
        pb.MainCommand(get_player_state=pb.GetPlayerStateRequest()),
        pb.MainCommandResponse(
            get_player_state=pb.GetPlayerStateResponse(state=pb.PlayerState.PLAYING)
        ),
    )
    entry.expect_command(
        pb.MainCommand(get_time=pb.GetTimeRequest()),
        pb.MainCommandResponse(get_time=pb.GetTimeResponse(time=5.0)),
    )
    entry.expect_command(
        pb.MainCommand(
            get_cache=pb.GetCacheRequest(key="discord_cover:linkin park meteora")
        ),
        pb.MainCommandResponse(get_cache=pb.GetCacheResponse()),
    )
    entry.expect_command(
        pb.MainCommand(
            batch_http_request=pb.BatchHttpRequest(
                requests=[
                    pb.HttpRequest(
                        url="https://api.deezer.com/search?q=Linkin+Park+Meteora&limit=1",
                        method="GET",
                    )
                ]
            )
        ),
        pb.MainCommandResponse(
            batch_http_request=pb.BatchHttpResponse(
                responses=[
                    pb.HttpResult(
                        response=pb.HttpResponse(status_code=200, body=deezer_body)
                    )
                ]
            )
        ),
    )
    entry.expect_command(
        pb.MainCommand(
            set_cache=pb.SetCacheRequest(
                key="discord_cover:linkin park meteora",
                data=cover_url.encode("utf-8"),
                ttl_seconds=86400,
            )
        ),
        pb.MainCommandResponse(set_cache=pb.SetCacheResponse(success=True)),
    )

    resp = entry.send_command(pb.ExtensionCommand(song_changed=pb.SongChangedRequest()))
    assert resp.HasField("song_changed")
    assert len(entry.manager.local_expectations) == 0


def test_local_song_imgbb_upload(entry: Moounit):
    thumb_dir = entry._mapped_paths.get("/thumbnails")
    assert thumb_dir is not None
    os.makedirs(thumb_dir, exist_ok=True)
    with open(os.path.join(thumb_dir, "cover_test.webp"), "wb") as f:
        f.write(b"img")

    local_song = songs_pb.Song(
        song=songs_pb.InnerSong(
            id="local:456",
            title="Faint",
            type=songs_pb.SongType.LOCAL,
            song_cover_path_high="/cache/moosync/thumbnails/cover_test.webp",
        ),
        artists=[songs_pb.Artist(artist_name="Linkin Park")],
        album=songs_pb.Album(album_name="Meteora"),
    )
    uploaded_url = "https://i.ibb.co/test/cover.webp"
    imgbb_body = json.dumps({"data": {"url": uploaded_url}, "success": True}).encode(
        "utf-8"
    )
    expected_b64 = base64.b64encode(b"img").decode("ascii")

    entry.expect_command(
        pb.MainCommand(get_current_song=pb.GetCurrentSongRequest()),
        pb.MainCommandResponse(
            get_current_song=pb.GetCurrentSongResponse(song=local_song)
        ),
    )
    entry.expect_command(
        pb.MainCommand(get_player_state=pb.GetPlayerStateRequest()),
        pb.MainCommandResponse(
            get_player_state=pb.GetPlayerStateResponse(state=pb.PlayerState.PLAYING)
        ),
    )
    entry.expect_command(
        pb.MainCommand(get_time=pb.GetTimeRequest()),
        pb.MainCommandResponse(get_time=pb.GetTimeResponse(time=5.0)),
    )
    entry.expect_command(
        pb.MainCommand(
            get_cache=pb.GetCacheRequest(key="discord_upload:cover_test.webp")
        ),
        pb.MainCommandResponse(get_cache=pb.GetCacheResponse()),
    )
    entry.expect_command(
        pb.MainCommand(
            get_preference=pb.GetPreferenceRequest(
                data=pb.PreferenceData(key="imgbb_api_key")
            )
        ),
        pb.MainCommandResponse(
            get_preference=pb.GetPreferenceResponse(
                data=pb.PreferenceData(
                    key="imgbb_api_key",
                    value=Value(string_value="test_imgbb_key"),
                )
            )
        ),
    )
    entry.expect_command(
        pb.MainCommand(
            batch_http_request=pb.BatchHttpRequest(
                requests=[
                    pb.HttpRequest(
                        url="https://api.imgbb.com/1/upload?key=test_imgbb_key&expiration=86400",
                        method="POST",
                        headers={"Content-Type": "application/x-www-form-urlencoded"},
                        body=f"image={expected_b64}".encode("utf-8"),
                    )
                ]
            )
        ),
        pb.MainCommandResponse(
            batch_http_request=pb.BatchHttpResponse(
                responses=[
                    pb.HttpResult(
                        response=pb.HttpResponse(status_code=200, body=imgbb_body)
                    )
                ]
            )
        ),
    )
    entry.expect_command(
        pb.MainCommand(
            set_cache=pb.SetCacheRequest(
                key="discord_upload:cover_test.webp",
                data=uploaded_url.encode("utf-8"),
                ttl_seconds=82800,
            )
        ),
        pb.MainCommandResponse(set_cache=pb.SetCacheResponse(success=True)),
    )

    resp = entry.send_command(pb.ExtensionCommand(song_changed=pb.SongChangedRequest()))
    assert resp.HasField("song_changed")
    assert len(entry.manager.local_expectations) == 0
