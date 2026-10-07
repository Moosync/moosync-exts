# pylint: disable=unused-import
import pytest
from google.protobuf.json_format import ParseDict
from core.types.protos.extensions_pb2 import (
    BatchHttpResponse,
    ExtensionCommand,
    ExtensionProviderScope,
    GetProviderScopesRequest,
    MainCommand,
    MainCommandResponse,
    OauthCallbackRequest,
    PerformAccountLoginRequest,
    PlaybackDetailsRequestedRequest,
    RequestedAlbumSongsRequest,
    RequestedArtistSongsRequest,
    RequestedPlaylistFromUrlRequest,
    RequestedPlaylistSongsRequest,
    RequestedPlaylistsRequest,
    RequestedRecommendationsRequest,
    RequestedSearchResultRequest,
    RequestedSongFromUrlRequest,
)
from core.types.protos.songs_pb2 import Album, Artist, InnerSong, Playlist, Song
from moounit import Moounit
from moounit.core import BatchHttpRequest


@pytest.fixture
def entry(moounit: Moounit):
    print("calling entry")
    moounit.replay_ignore(
        request=BatchHttpRequest(),
        ignore_fields=[
            "headers.X-Goog-Visitor-Id",
            "headers.user-agent",
        ],
    )
    moounit.replay_ignore(
        request=BatchHttpRequest(),
        ignore_fields=["body"],
        regex=r'"(visitorData|userAgent)":\s*"[^"]*"',
    )
    moounit.call_entry()
    return moounit


def test_get_provider_scopes(entry: Moounit):
    scopes = list(entry.manifest.scopes)
    scopes.sort()

    assert len(scopes) == 10
    expected = [
        ExtensionProviderScope.ACCOUNTS,
        ExtensionProviderScope.ALBUM_SONGS,
        ExtensionProviderScope.ARTIST_SONGS,
        ExtensionProviderScope.PLAYBACK_DETAILS,
        ExtensionProviderScope.PLAYLISTS,
        ExtensionProviderScope.PLAYLIST_FROM_URL,
        ExtensionProviderScope.PLAYLIST_SONGS,
        ExtensionProviderScope.RECOMMENDATIONS,
        ExtensionProviderScope.SEARCH,
        ExtensionProviderScope.SONG_FROM_URL,
    ]
    expected.sort()
    assert scopes == expected


def test_search(entry: Moounit):
    entry.replay_ignore(
        request=BatchHttpRequest(),
        ignore_field=[
            "headers.X-Goog-Visitor-Id",
            "body",
            "headers.user-agent",
        ],
    )
    ret = entry.send_command(
        ExtensionCommand(
            requested_search_result=RequestedSearchResultRequest(query="test")
        )
    )
    songs = ret.requested_search_result.songs
    assert len(songs) == 30

    first = songs[0]
    assert first.song.id == "moosync.youtube.js:youtube:kLCx86Uq58w"
    assert (
        first.song.title
        == "Ultimate Four Frequency Sound Check Deep Bass, Stereo, LR Channel & Speaker Test"
    )
    assert first.song.duration.seconds == 162
    assert first.song.url == "https://www.youtube.com/watch?v=kLCx86Uq58w"
    assert first.song.song_cover_path_high.startswith("http")
    assert first.song.song_cover_path_low.startswith("http")
    assert first.album.album_id == "moosync.youtube.js:youtube-album:MPREb_gCVRMLxJPTV"
    assert (
        first.album.album_name
        == "Ultimate Four Frequency Sound Check Deep Bass, Stereo, LR Channel & Speaker Test"
    )
    assert len(first.artists) == 1
    assert (
        first.artists[0].artist_id
        == "moosync.youtube.js:youtube-artist:UCLS1b-9ruWzU2XHjkTvmhdw"
    )
    assert first.artists[0].artist_name == "DJ LNS"

    second = songs[1]
    assert second.song.id == "moosync.youtube.js:youtube:x8ovxXV7krw"
    assert second.song.title == "Extreme Bass Impact - Sound Test"
    assert second.song.duration.seconds == 330
    assert second.song.url == "https://www.youtube.com/watch?v=x8ovxXV7krw"
    assert second.album.album_id == "moosync.youtube.js:youtube-album:MPREb_N8XI8uiOaub"
    assert second.album.album_name == "Extreme Bass Impact - Sound Test"
    assert len(second.artists) == 1
    assert (
        second.artists[0].artist_id
        == "moosync.youtube.js:youtube-artist:UCAd6HZUQ2O4ESM42hk0rCNw"
    )
    assert second.artists[0].artist_name == "Fabrício Cesar"


def test_song_from_url(entry: Moounit):
    entry.replay_ignore(
        request=BatchHttpRequest(),
        ignore_field=[
            "headers.X-Goog-Visitor-Id",
            "body",
            "headers.user-agent",
        ],
    )
    ret = entry.send_command(
        ExtensionCommand(
            requested_song_from_url=RequestedSongFromUrlRequest(
                url="https://www.youtube.com/watch?v=dQw4w9WgXcQ"
            )
        )
    )
    song_detail = ret.requested_song_from_url.song
    assert song_detail.song.id == "moosync.youtube.js:youtube:dQw4w9WgXcQ"
    assert song_detail.song.title == "Never Gonna Give You Up"
    assert song_detail.song.duration.seconds == 213
    assert song_detail.song.url == "https://www.youtube.com/watch?v=dQw4w9WgXcQ"
    assert song_detail.song.song_cover_path_high.startswith("http")
    assert song_detail.song.song_cover_path_low.startswith("http")
    assert len(song_detail.artists) == 1
    assert (
        song_detail.artists[0].artist_id
        == "moosync.youtube.js:youtube-artist:UCuAXFkgsw1L7xaCfnd5JJOw"
    )
    assert song_detail.artists[0].artist_name == "Rick Astley"


def test_playlist_from_url(entry: Moounit):
    entry.replay_ignore(
        request=BatchHttpRequest(),
        ignore_field=[
            "headers.X-Goog-Visitor-Id",
            "body",
            "headers.user-agent",
        ],
    )
    ret = entry.send_command(
        ExtensionCommand(
            requested_playlist_from_url=RequestedPlaylistFromUrlRequest(
                url="https://music.youtube.com/playlist?list=PLDIoUOhQQPlU2DoseElwLlKXmV9YSEnKI"
            )
        )
    )
    songs = ret.requested_playlist_from_url.songs
    assert len(songs) == 98

    first = songs[0]
    assert first.song.id == "moosync.youtube.js:youtube:EFqvVSqh6R8"
    assert first.song.title == "Next Summer (Official Video)"
    assert first.song.duration.seconds == 313
    assert first.song.url == "https://www.youtube.com/watch?v=EFqvVSqh6R8"

    second = songs[1]
    assert second.song.id == "moosync.youtube.js:youtube:ko70cExuzZM"
    assert second.song.title == "The Fate of Ophelia"
    assert second.song.duration.seconds == 239
    assert second.song.url == "https://www.youtube.com/watch?v=ko70cExuzZM"


def test_playlist_songs(entry: Moounit):
    entry.replay_ignore(
        request=BatchHttpRequest(),
        ignore_field=[
            "headers.X-Goog-Visitor-Id",
            "body",
            "headers.user-agent",
        ],
    )
    ret = entry.send_command(
        ExtensionCommand(
            requested_playlist_songs=RequestedPlaylistSongsRequest(
                id="youtube-playlist:PLDIoUOhQQPlU2DoseElwLlKXmV9YSEnKI"
            )
        )
    )
    songs = ret.requested_playlist_songs.songs
    assert len(songs) == 98

    first = songs[0]
    assert first.song.id == "moosync.youtube.js:youtube:EFqvVSqh6R8"
    assert first.song.title == "Next Summer (Official Video)"
    assert first.song.duration.seconds == 313
    assert first.song.url == "https://www.youtube.com/watch?v=EFqvVSqh6R8"
    assert first.song.song_cover_path_high.startswith("http")
    assert first.song.song_cover_path_low.startswith("http")

    second = songs[1]
    assert second.song.id == "moosync.youtube.js:youtube:ko70cExuzZM"
    assert second.song.title == "The Fate of Ophelia"
    assert second.song.duration.seconds == 239
    assert second.song.url == "https://www.youtube.com/watch?v=ko70cExuzZM"


def test_artist_songs(entry: Moounit):
    entry.replay_ignore(
        request=BatchHttpRequest(),
        ignore_field=[
            "headers.X-Goog-Visitor-Id",
            "body",
            "headers.user-agent",
        ],
    )
    ret = entry.send_command(
        ExtensionCommand(
            requested_artist_songs=RequestedArtistSongsRequest(
                artist=Artist(artist_id="youtube-artist:UCedvOgsKFzcK3hA5taf3KoQ")
            )
        )
    )
    songs = ret.requested_artist_songs.songs
    assert len(songs) == 100

    first = songs[0]
    assert first.song.id == "moosync.youtube.js:youtube:9kznlAwE-8o"
    assert first.song.title == "Mockingbird"
    assert first.song.duration.seconds == 251
    assert first.song.url == "https://www.youtube.com/watch?v=9kznlAwE-8o"
    assert len(first.artists) == 1
    assert (
        first.artists[0].artist_id
        == "moosync.youtube.js:youtube-artist:UCedvOgsKFzcK3hA5taf3KoQ"
    )
    assert first.artists[0].artist_name == "Eminem"

    second = songs[1]
    assert second.song.id == "moosync.youtube.js:youtube:RnkShwdXfyc"
    assert second.song.title == "Love The Way You Lie (feat. Rihanna)"
    assert second.song.duration.seconds == 264
    assert second.song.url == "https://www.youtube.com/watch?v=RnkShwdXfyc"
    assert len(second.artists) == 1
    assert (
        second.artists[0].artist_id
        == "moosync.youtube.js:youtube-artist:UCedvOgsKFzcK3hA5taf3KoQ"
    )
    assert second.artists[0].artist_name == "Eminem"


def test_album_songs(entry: Moounit):
    entry.replay_ignore(
        request=BatchHttpRequest(),
        ignore_field=[
            "headers.X-Goog-Visitor-Id",
            "body",
            "headers.user-agent",
        ],
    )
    ret = entry.send_command(
        ExtensionCommand(
            requested_album_songs=RequestedAlbumSongsRequest(
                album=Album(album_id="youtube-album:MPREb_GOUuLzrL1Wd")
            )
        )
    )
    songs = ret.requested_album_songs.songs
    assert len(songs) == 1

    first = songs[0]
    assert first.song.id == "moosync.youtube.js:youtube:e_CrkgWHEB0"
    assert first.song.title == "Nevermind, Not Nirvana"
    assert first.song.duration.seconds == 147
    assert first.song.url == "https://www.youtube.com/watch?v=e_CrkgWHEB0"


def test_recommendations(entry: Moounit):
    entry.replay_ignore(
        request=BatchHttpRequest(),
        ignore_field=[
            "headers.X-Goog-Visitor-Id",
            "body",
            "headers.user-agent",
        ],
    )
    ret = entry.send_command(
        ExtensionCommand(requested_recommendations=RequestedRecommendationsRequest())
    )
    songs = ret.requested_recommendations.songs
    assert len(songs) == 68

    first = songs[0]
    assert first.song.id == "moosync.youtube.js:youtube:MPREb_opei2OHmlwY"
    assert first.song.title == "Fck the Family Name (feat. Ana Ballad)"
    assert first.song.url == "https://www.youtube.com/watch?v=MPREb_opei2OHmlwY"
    assert len(first.artists) == 1
    assert (
        first.artists[0].artist_id
        == "moosync.youtube.js:youtube-artist:UCAmgqptIvuZCvst0u6t7sHw"
    )
    assert first.artists[0].artist_name == "Seth Ballad"


def test_oauth_flow(entry: Moounit):
    entry.replay_ignore(
        request=BatchHttpRequest(),
        ignore_field=[
            "headers.X-Goog-Visitor-Id",
            "body",
            "headers.user-agent",
        ],
    )
    entry.replay_ignore(
        request=MainCommand(),
        ignore_fields=["set_secure.data.value.string_value"],
    )
    ret = entry.send_command(
        ExtensionCommand(
            perform_account_login=PerformAccountLoginRequest(login_status=True)
        )
    )
    assert ret.perform_account_login.status == "ZXS-XFK-XTHK"

    cb_ret = entry.send_command(
        ExtensionCommand(oauth_callback=OauthCallbackRequest(callback_uri="done"))
    )
    assert cb_ret.oauth_callback is not None

    account = entry.accounts.get("youtube")
    assert account is not None
    assert account.id == "youtube"
    assert account.name == "YouTube"
    assert account.package_name == "moosync.youtube.js"
    assert account.bg_color == "#FF0000"
    assert account.logged_in is True
    assert account.username == "Sahil Gupte"


def test_requested_playlists(entry: Moounit):
    entry.replay_ignore(
        request=BatchHttpRequest(),
        ignore_field=[
            "headers.X-Goog-Visitor-Id",
            "body",
            "headers.user-agent",
        ],
    )
    entry.replay_ignore(
        request=MainCommand(),
        ignore_fields=["set_secure.data.value.string_value"],
    )

    oauth_calls = entry.recorder.replay_test_calls.get(
        "test_oauth_flow"
    ) or entry.recorder.existing_data.get("tests", {}).get("test_oauth_flow", [])
    for call in oauth_calls:
        if call.get("type") == "batch_http_request":
            req = ParseDict(call["request"], BatchHttpRequest())
            resp = ParseDict(call["response"], BatchHttpResponse())
            entry.expect_batch_http_request(req, resp)
        elif call.get("type") == "send_main_command":
            cmd = ParseDict(call["request"], MainCommand())
            resp = ParseDict(call["response"], MainCommandResponse())
            entry.expect_command(cmd, resp)

    ret = entry.send_command(
        ExtensionCommand(
            perform_account_login=PerformAccountLoginRequest(login_status=True)
        )
    )
    assert ret.perform_account_login.status == "ZXS-XFK-XTHK"

    cb_ret = entry.send_command(
        ExtensionCommand(oauth_callback=OauthCallbackRequest(callback_uri="done"))
    )
    assert cb_ret.oauth_callback is not None

    account = entry.accounts.get("youtube")
    assert account is not None
    assert account.logged_in is True

    pl_ret = entry.send_command(
        ExtensionCommand(requested_playlists=RequestedPlaylistsRequest())
    )
    playlists = pl_ret.requested_playlists.playlists
    assert len(playlists) == 2

    first = playlists[0]
    assert (
        first.playlist_id
        == "moosync.youtube.js:youtube-playlist:PLDIoUOhQQPlU2DoseElwLlKXmV9YSEnKI"
    )
    assert first.playlist_name == "Liked Music"
    assert first.playlist_song_count == 98
    assert (
        first.playlist_coverpath == "https://i.ytimg.com/vi/EFqvVSqh6R8/hqdefault.jpg"
    )

    second = playlists[1]
    assert (
        second.playlist_id
        == "moosync.youtube.js:youtube-playlist:PLO0BbVUKhzndecKXOAEm3I76w46RNLkgK"
    )
    assert second.playlist_name == "kek"
    assert second.playlist_song_count == 25
    assert (
        second.playlist_coverpath == "https://i.ytimg.com/vi/R_7eZYhkcOg/hqdefault.jpg"
    )


def test_playback_details(entry: Moounit):
    entry.replay_ignore(
        request=BatchHttpRequest(),
        ignore_field=[
            "headers.X-Goog-Visitor-Id",
            "body",
            "headers.user-agent",
        ],
    )
    ret = entry.send_command(
        ExtensionCommand(
            playback_details_requested=PlaybackDetailsRequestedRequest(
                song=Song(song=InnerSong(id="moosync.youtube_js:dQw4w9WgXcQ"))
            )
        )
    )
    playback = ret.playback_details_requested
    assert playback.url.startswith("https://")
    assert "googlevideo.com" in playback.url
    assert playback.url.endswith(".m3u8")
    assert playback.duration.seconds == 213
