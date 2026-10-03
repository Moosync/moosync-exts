# pylint: disable=no-member,unused-import
import pytest
from core.types.protos.extensions_pb2 import (
    BatchHttpRequest,
    ExtensionAccountDetail,
    ExtensionCommand,
    ExtensionProviderScope,
    GetProviderScopesRequest,
    GetSecureRequest,
    GetSecureResponse,
    MainCommand,
    MainCommandResponse,
    PlaybackDetailsRequestedRequest,
    PreferenceData,
    RegisterUserPreferenceRequest,
    RegisterUserPreferenceResponse,
    RequestedAlbumSongsRequest,
    RequestedArtistSongsRequest,
    RequestedPlaylistFromUrlRequest,
    RequestedPlaylistSongsRequest,
    RequestedRecommendationsRequest,
    RequestedSearchResultRequest,
    RequestedSongFromIdRequest,
    RequestedSongFromUrlRequest,
    SetAccountRequest,
    SetAccountResponse,
)
from core.types.protos.songs_pb2 import Album, Artist, InnerSong, Playlist, Song
import core.types.protos.preferences_pb2 as pref_pb
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
    moounit.expect_command(
        MainCommand(
            get_secure=GetSecureRequest(data=PreferenceData(key="media_user_token"))
        ),
        MainCommandResponse(get_secure=GetSecureResponse(data=PreferenceData())),
    )
    moounit.expect_command(
        MainCommand(
            set_account=SetAccountRequest(
                account=ExtensionAccountDetail(
                    id="apple_music",
                    package_name="moosync.apple.music",
                    name="Apple Music",
                    bg_color="#FA233B",
                    logged_in=False,
                    icon="",
                )
            )
        ),
        MainCommandResponse(set_account=SetAccountResponse()),
    )
    moounit.expect_command(
        MainCommand(
            register_user_preference=RegisterUserPreferenceRequest(
                prefs=[
                    pref_pb.PreferenceItem(
                        pref_type=pref_pb.PreferenceType.PREFERENCE_TYPE_TEXT_INPUT_GROUP,
                        title="Media User Token",
                        id="media_user_token",
                        subtitle="Apple Music account media-user-token cookie",
                    )
                ]
            )
        ),
        MainCommandResponse(register_user_preference=RegisterUserPreferenceResponse()),
    )
    moounit.call_entry()
    return moounit


def test_get_provider_scopes(entry: Moounit):
    ret = entry.send_command(
        ExtensionCommand(get_provider_scopes=GetProviderScopesRequest())
    )

    scopes = ret.get_provider_scopes.scopes
    scopes.sort()

    # Check that our active scopes are reported
    assert ExtensionProviderScope.SEARCH in scopes
    assert ExtensionProviderScope.SONG_FROM_URL in scopes
    assert ExtensionProviderScope.PLAYBACK_DETAILS in scopes
    assert ExtensionProviderScope.ALBUM_SONGS in scopes
    assert ExtensionProviderScope.ARTIST_SONGS in scopes
    assert ExtensionProviderScope.PLAYLIST_SONGS in scopes
    assert ExtensionProviderScope.RECOMMENDATIONS in scopes


def test_search(entry: Moounit):
    ret = entry.send_command(
        ExtensionCommand(
            requested_search_result=RequestedSearchResultRequest(
                query="Michael Jackson Billie Jean"
            )
        )
    )

    songs = ret.requested_search_result.songs
    assert len(songs) == 25

    first = songs[0]
    assert first.song.id == "applemusic:269573364"
    assert first.song.title == "Billie Jean"
    assert first.song.duration.seconds == 293
    assert first.album.album_name == "Thriller"
    assert len(first.artists) == 1
    assert first.artists[0].artist_name == "Michael Jackson"

    second = songs[1]
    assert second.song.id == "applemusic:273048805"
    assert second.song.title == "Billie Jean (1981 Home Demo)"
    assert second.song.duration.seconds == 140
    assert second.album.album_name == "Thriller (25th Anniversary) [Deluxe Edition]"


def test_song_from_url(entry: Moounit):
    ret = entry.send_command(
        ExtensionCommand(
            requested_song_from_url=RequestedSongFromUrlRequest(
                url="https://music.apple.com/us/album/billie-jean/269572838?i=269573364"
            )
        )
    )

    assert ret.requested_song_from_url.song is not None
    song = ret.requested_song_from_url.song
    assert song.song.id == "applemusic:269573364"
    assert song.song.title == "Billie Jean"
    assert song.song.duration.seconds == 293
    assert song.album.album_name == "Thriller"
    assert len(song.artists) == 1
    assert song.artists[0].artist_name == "Michael Jackson"
    assert song.song.playback_url.startswith("https://audio-ssl.itunes.apple.com/")


def test_song_from_id(entry: Moounit):
    ret = entry.send_command(
        ExtensionCommand(
            requested_song_from_id=RequestedSongFromIdRequest(id="applemusic:269573364")
        )
    )

    assert ret.requested_song_from_id.song is not None
    song = ret.requested_song_from_id.song
    assert song.song.id == "applemusic:269573364"
    assert song.song.title == "Billie Jean"
    assert song.song.duration.seconds == 293
    assert song.album.album_name == "Thriller"
    assert len(song.artists) == 1
    assert song.artists[0].artist_name == "Michael Jackson"


def test_playlist_from_url(entry: Moounit):
    url = (
        "https://music.apple.com/us/playlist/todays-hits/"
        "pl.f4d106fed2bd41149aaacabb233eb5eb"
    )
    ret = entry.send_command(
        ExtensionCommand(
            requested_playlist_from_url=RequestedPlaylistFromUrlRequest(url=url)
        )
    )

    assert ret.requested_playlist_from_url.playlist is not None
    playlist = ret.requested_playlist_from_url.playlist
    assert (
        playlist.playlist_id
        == "applemusic:playlist:pl.f4d106fed2bd41149aaacabb233eb5eb"
    )
    assert playlist.playlist_name == "Today’s Hits"


def test_playlist_songs(entry: Moounit):
    ret = entry.send_command(
        ExtensionCommand(
            requested_playlist_songs=RequestedPlaylistSongsRequest(
                id="pl.f4d106fed2bd41149aaacabb233eb5eb"
            )
        )
    )

    songs = ret.requested_playlist_songs.songs
    assert len(songs) == 50
    first = songs[0]
    assert first.song.id == "applemusic:6818209989"
    assert first.song.title == "Solar Eclipse"
    assert first.song.duration.seconds == 218
    assert len(first.artists) >= 1
    assert "Drake" in first.artists[0].artist_name


def test_album_songs(entry: Moounit):
    ret = entry.send_command(
        ExtensionCommand(
            requested_album_songs=RequestedAlbumSongsRequest(
                album=Album(album_id="applemusic:album:269572838")
            )
        )
    )

    songs = ret.requested_album_songs.songs
    assert len(songs) == 9
    first = songs[0]
    assert first.song.id == "applemusic:269572927"
    assert first.song.title == "Wanna Be Startin' Somethin'"
    assert first.song.duration.seconds == 363
    assert first.song.track_no == 1.0

    billie_jean = next(s for s in songs if s.song.id == "applemusic:269573364")
    assert billie_jean.song.title == "Billie Jean"
    assert billie_jean.song.duration.seconds == 293
    assert billie_jean.song.track_no == 6.0


def test_artist_songs(entry: Moounit):
    ret = entry.send_command(
        ExtensionCommand(
            requested_artist_songs=RequestedArtistSongsRequest(
                artist=Artist(artist_id="applemusic:artist:32940")
            )
        )
    )

    songs = ret.requested_artist_songs.songs
    assert len(songs) == 50
    first = songs[0]
    assert first.song.id == "applemusic:269573303"
    assert first.song.title == "Thriller"
    assert first.song.duration.seconds == 358
    assert len(first.artists) == 1
    assert first.artists[0].artist_name == "Michael Jackson"


def test_recommendations(entry: Moounit):
    ret = entry.send_command(
        ExtensionCommand(requested_recommendations=RequestedRecommendationsRequest())
    )

    songs = ret.requested_recommendations.songs
    assert len(songs) == 25
    first = songs[0]
    assert first.song.id == "applemusic:6818209989"
    assert first.song.title == "Solar Eclipse"


def test_playback_details(entry: Moounit):
    ret = entry.send_command(
        ExtensionCommand(
            playback_details_requested=PlaybackDetailsRequestedRequest(
                song=Song(song=InnerSong(id="applemusic:269573364"))
            )
        )
    )

    resp = ret.playback_details_requested
    assert resp.url.startswith("https://audio-ssl.itunes.apple.com/")
    assert ".m4a" in resp.url
    assert resp.duration.seconds == 293
