import pytest
from core.types.protos.extensions_pb2 import (
    BatchHttpRequest,
    ExtensionCommand,
    ExtensionProviderScope,
    GetCacheRequest,
    GetCacheResponse,
    MainCommand,
    MainCommandResponse,
    RequestedCoverArtRequest,
    RequestedCoverArtResponse,
)
from core.types.protos.songs_pb2 import Album, Artist, InnerSong, Song
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
    assert scopes == [ExtensionProviderScope.COVER_ART]


def test_get_cover_art_song_standard(entry: Moounit):
    test_song = Song(
        song=InnerSong(
            title="Yesterday",
        ),
        artists=[Artist(artist_name="The Beatles")],
    )

    ret = entry.send_command(
        ExtensionCommand(requested_cover_art=RequestedCoverArtRequest(song=test_song))
    )

    assert ret.HasField("requested_cover_art")
    assert len(ret.requested_cover_art.cover_urls) > 0


def test_get_cover_art_song_common_artist(entry: Moounit):
    test_song = Song(
        song=InnerSong(
            title="Home",
        ),
        artists=[Artist(artist_name="Michael Bublé")],
    )

    ret = entry.send_command(
        ExtensionCommand(requested_cover_art=RequestedCoverArtRequest(song=test_song))
    )

    assert ret.HasField("requested_cover_art")
    assert len(ret.requested_cover_art.cover_urls) > 0


def test_get_cover_art_song_non_existent(entry: Moounit):
    test_song = Song(
        song=InnerSong(
            title="NonExistentTrack_XYZ_12345",
        ),
        artists=[Artist(artist_name="NonExistentArtist_XYZ_98765")],
    )

    ret = entry.send_command(
        ExtensionCommand(requested_cover_art=RequestedCoverArtRequest(song=test_song))
    )

    assert ret.HasField("requested_cover_art")
    assert len(ret.requested_cover_art.cover_urls) == 0


def test_get_cover_art_album_standard(entry: Moounit):
    test_album = Album(
        album_name="Abbey Road",
        album_artist="The Beatles",
    )

    ret = entry.send_command(
        ExtensionCommand(requested_cover_art=RequestedCoverArtRequest(album=test_album))
    )

    assert ret.HasField("requested_cover_art")
    assert len(ret.requested_cover_art.cover_urls) > 0


def test_get_cover_art_album_common_name(entry: Moounit):
    test_album = Album(
        album_name="Greatest Hits",
        album_artist="Queen",
    )

    ret = entry.send_command(
        ExtensionCommand(requested_cover_art=RequestedCoverArtRequest(album=test_album))
    )

    assert ret.HasField("requested_cover_art")
    assert len(ret.requested_cover_art.cover_urls) > 0


def test_get_cover_art_album_non_existent(entry: Moounit):
    test_album = Album(
        album_name="NonExistentAlbum_XYZ_12345",
        album_artist="NonExistentArtist_XYZ_98765",
    )

    ret = entry.send_command(
        ExtensionCommand(requested_cover_art=RequestedCoverArtRequest(album=test_album))
    )

    assert ret.HasField("requested_cover_art")
    assert len(ret.requested_cover_art.cover_urls) == 0


def test_get_cover_art_artist_standard(entry: Moounit):
    test_artist = Artist(
        artist_name="The Beatles",
    )

    ret = entry.send_command(
        ExtensionCommand(
            requested_cover_art=RequestedCoverArtRequest(artist=test_artist)
        )
    )

    assert ret.HasField("requested_cover_art")
    assert len(ret.requested_cover_art.cover_urls) > 0


def test_get_cover_art_artist_common_name(entry: Moounit):
    test_artist = Artist(
        artist_name="John",
    )

    ret = entry.send_command(
        ExtensionCommand(
            requested_cover_art=RequestedCoverArtRequest(artist=test_artist)
        )
    )

    assert ret.HasField("requested_cover_art")
    assert len(ret.requested_cover_art.cover_urls) > 0


def test_get_cover_art_artist_non_existent(entry: Moounit):
    test_artist = Artist(
        artist_name="NonExistentArtist_XYZ_98765",
    )

    ret = entry.send_command(
        ExtensionCommand(
            requested_cover_art=RequestedCoverArtRequest(artist=test_artist)
        )
    )

    assert ret.HasField("requested_cover_art")
    assert len(ret.requested_cover_art.cover_urls) == 0


def test_get_cover_art_empty_request(entry: Moounit):
    ret = entry.send_command(
        ExtensionCommand(requested_cover_art=RequestedCoverArtRequest())
    )

    assert ret.HasField("requested_cover_art")
    assert len(ret.requested_cover_art.cover_urls) == 0


def test_get_cover_art_cache_hit(entry: Moounit):
    cached_response = RequestedCoverArtResponse(
        cover_urls=["https://example.com/cached_cover.jpg"]
    )
    entry.expect_command(
        command=MainCommand(
            get_cache=GetCacheRequest(key="cover:song:cached track:cached artist")
        ),
        response=MainCommandResponse(
            get_cache=GetCacheResponse(data=cached_response.SerializeToString())
        ),
    )

    test_song = Song(
        song=InnerSong(title="Cached Track"),
        artists=[Artist(artist_name="Cached Artist")],
    )

    ret = entry.send_command(
        ExtensionCommand(requested_cover_art=RequestedCoverArtRequest(song=test_song))
    )

    assert ret.HasField("requested_cover_art")
    assert list(ret.requested_cover_art.cover_urls) == [
        "https://example.com/cached_cover.jpg"
    ]
