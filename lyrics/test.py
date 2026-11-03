import pytest
from core.types.protos.extensions_pb2 import (
    BatchHttpRequest,
    ExtensionCommand,
    ExtensionProviderScope,
    RequestedLyricsRequest,
)
from core.types.protos.songs_pb2 import Artist, InnerSong, Song
from moounit import Moounit


@pytest.fixture
def entry(moounit: Moounit):
    moounit.replay_ignore(
        request=BatchHttpRequest(),
        ignore_fields=[
            "headers.user-agent",
            "headers.User-Agent",
        ],
    )
    try:
        moounit.call_entry()
    except Exception as e:
        print(f"Failed to call entry: {e}")
        raise e
    return moounit


def test_get_provider_scopes(entry: Moounit):
    scopes = list(entry.manifest.scopes)
    scopes.sort()

    assert len(scopes) == 1
    assert scopes == [ExtensionProviderScope.LYRICS]


def test_get_lyrics_song_standard(entry: Moounit):
    test_song = Song(
        song=InnerSong(
            title="Yesterday",
        ),
        artists=[Artist(artist_name="The Beatles")],
    )

    ret = entry.send_command(
        ExtensionCommand(requested_lyrics=RequestedLyricsRequest(song=test_song))
    )

    assert ret.HasField("requested_lyrics")
    assert ret.requested_lyrics.HasField("lyrics")
    assert len(ret.requested_lyrics.lyrics.lines) > 0


def test_get_lyrics_song_common_artist(entry: Moounit):
    test_song = Song(
        song=InnerSong(
            title="Hello",
        ),
        artists=[Artist(artist_name="Adele")],
    )

    ret = entry.send_command(
        ExtensionCommand(requested_lyrics=RequestedLyricsRequest(song=test_song))
    )

    assert ret.HasField("requested_lyrics")
    assert ret.requested_lyrics.HasField("lyrics")
    assert len(ret.requested_lyrics.lyrics.lines) > 0


def test_get_lyrics_song_non_existent(entry: Moounit):
    test_song = Song(
        song=InnerSong(
            title="NonExistentSongTitle_XYZ_12345",
        ),
        artists=[Artist(artist_name="NonExistentArtist_XYZ_98765")],
    )

    ret = entry.send_command(
        ExtensionCommand(requested_lyrics=RequestedLyricsRequest(song=test_song))
    )

    assert ret.HasField("requested_lyrics")
    assert not ret.requested_lyrics.HasField("lyrics")


def test_get_lyrics_song_empty_title(entry: Moounit):
    test_song = Song(
        song=InnerSong(
            title="",
        ),
        artists=[Artist(artist_name="The Beatles")],
    )

    ret = entry.send_command(
        ExtensionCommand(requested_lyrics=RequestedLyricsRequest(song=test_song))
    )

    assert ret.HasField("requested_lyrics")
    assert not ret.requested_lyrics.HasField("lyrics")


def test_get_lyrics_empty_song(entry: Moounit):
    ret = entry.send_command(
        ExtensionCommand(requested_lyrics=RequestedLyricsRequest(song=Song()))
    )

    assert ret.HasField("requested_lyrics")
    assert not ret.requested_lyrics.HasField("lyrics")
