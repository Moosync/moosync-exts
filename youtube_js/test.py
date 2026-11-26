# pylint: disable=unused-import
import pytest
from google.protobuf.duration_pb2 import Duration
from google.protobuf.json_format import ParseDict
from core.types.protos.extensions_pb2 import (
    BatchHttpRequest,
    BatchHttpResponse,
    ExtensionCommand,
    ExtensionProviderScope,
    GetCacheRequest,
    GetCacheResponse,
    GetProviderScopesRequest,
    GetSecureRequest,
    GetSecureResponse,
    MainCommand,
    MainCommandResponse,
    OauthCallbackRequest,
    PerformAccountLoginRequest,
    PlaybackDetailsRequestedRequest,
    PlaybackDetailsRequestedResponse,
    PreferenceData,
    RegisterOauthRequest,
    RegisterOauthResponse,
    RequestedAlbumSongsRequest,
    RequestedArtistSongsRequest,
    RequestedPlaylistSongsRequest,
    RequestedPlaylistsRequest,
    RequestedRecommendationsRequest,
    RequestedSearchResultRequest,
    RequestedSearchResultResponse,
    SetCacheRequest,
    SetCacheResponse,
)
from core.types.protos.songs_pb2 import Album, Artist, InnerSong, Playlist, Song
from moounit import Moounit


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
    moounit.expect_command(
        MainCommand(
            get_secure=GetSecureRequest(data=PreferenceData(key="youtube_oauth_tokens"))
        ),
        MainCommandResponse(
            get_secure=GetSecureResponse(
                data=PreferenceData(key="youtube_oauth_tokens")
            )
        ),
        times=-1,
    )
    moounit.expect_command(
        MainCommand(register_oauth=RegisterOauthRequest(url="app.moosync.youtube.js")),
        MainCommandResponse(register_oauth=RegisterOauthResponse(success=True)),
        times=-1,
    )
    moounit.call_entry()
    return moounit


def test_get_provider_scopes(entry: Moounit):
    scopes = sorted(s.scope for s in entry.manifest.scopes)

    assert len(scopes) == 8
    expected = sorted(
        [
            ExtensionProviderScope.ACCOUNTS,
            ExtensionProviderScope.ALBUM_SONGS,
            ExtensionProviderScope.ARTIST_SONGS,
            ExtensionProviderScope.PLAYBACK_DETAILS,
            ExtensionProviderScope.PLAYLISTS,
            ExtensionProviderScope.PLAYLIST_SONGS,
            ExtensionProviderScope.RECOMMENDATIONS,
            ExtensionProviderScope.SEARCH,
        ]
    )
    assert scopes == expected


def test_search(entry: Moounit):
    entry.replay_ignore(
        request=BatchHttpRequest(),
        ignore_fields=[
            "headers.X-Goog-Visitor-Id",
            "body",
            "headers.user-agent",
        ],
    )
    entry.replay_ignore(
        request=SetCacheRequest(),
        ignore_fields=["data"],
    )
    ret = entry.send_command(
        ExtensionCommand(
            requested_search_result=RequestedSearchResultRequest(query="test")
        )
    )
    songs = ret.requested_search_result.songs
    assert len(songs) == 30

    first = songs[0]
    assert first.song.id == "app.moosync.youtube.js:youtube:V2c3KVGv0X0"
    assert first.song.title == "Tested And True"
    assert first.song.duration.seconds == 480
    assert first.song.url == "https://www.youtube.com/watch?v=V2c3KVGv0X0"
    assert first.song.song_cover_path_high.startswith("http")
    assert first.song.song_cover_path_low.startswith("http")
    assert first.album.album_id == "app.moosync.youtube.js:youtube-album:MPREb_4sdpVL4FiGh"
    assert first.album.album_name == "Tested And True"
    assert len(first.artists) == 1
    assert (
        first.artists[0].artist_id
        == "app.moosync.youtube.js:youtube-artist:UCSxcflkwVWa_U6QtnIfvBeA"
    )
    assert first.artists[0].artist_name == "King Terryo"

    second = songs[1]
    assert second.song.id == "app.moosync.youtube.js:youtube:MqyPl1Kdw7k"
    assert second.song.title == "Ba Ra Ta Sound Test (feat. DJ FRANCIS ENERIDO)"
    assert second.song.duration.seconds == 181
    assert second.song.url == "https://www.youtube.com/watch?v=MqyPl1Kdw7k"
    assert second.album.album_id == "app.moosync.youtube.js:youtube-album:MPREb_YsapymF5Ag4"
    assert second.album.album_name == "Ba Ra Ta Sound Test"

    cached_search_resp = RequestedSearchResultResponse(
        songs=[
            Song(
                song=InnerSong(id="youtube:cached123", title="Cached Search Hit"),
            )
        ]
    )
    entry.expect_command(
        MainCommand(get_cache=GetCacheRequest(key="search:bass test")),
        MainCommandResponse(
            get_cache=GetCacheResponse(data=cached_search_resp.SerializeToString())
        ),
    )
    cached_ret = entry.send_command(
        ExtensionCommand(
            requested_search_result=RequestedSearchResultRequest(query="bass test")
        )
    )
    assert len(cached_ret.requested_search_result.songs) == 1
    assert cached_ret.requested_search_result.songs[0].song.title == "Cached Search Hit"


def test_playlist_songs(entry: Moounit):
    entry.replay_ignore(
        request=BatchHttpRequest(),
        ignore_fields=[
            "headers.X-Goog-Visitor-Id",
            "body",
            "headers.user-agent",
        ],
    )
    entry.replay_ignore(
        request=SetCacheRequest(),
        ignore_fields=["data"],
    )
    ret = entry.send_command(
        ExtensionCommand(
            requested_playlist_songs=RequestedPlaylistSongsRequest(
                id="youtube-playlist:PLDIoUOhQQPlU2DoseElwLlKXmV9YSEnKI"
            )
        )
    )
    songs = ret.requested_playlist_songs.songs
    assert len(songs) == 97

    first = songs[0]
    assert first.song.id == "app.moosync.youtube.js:youtube:EFqvVSqh6R8"
    assert first.song.title == "Next Summer (Official Video)"
    assert first.song.duration.seconds == 313
    assert first.song.url == "https://www.youtube.com/watch?v=EFqvVSqh6R8"
    assert first.song.song_cover_path_high.startswith("http")
    assert first.song.song_cover_path_low.startswith("http")

    second = songs[1]
    assert second.song.id == "app.moosync.youtube.js:youtube:ko70cExuzZM"
    assert second.song.title == "The Fate of Ophelia"
    assert second.song.duration.seconds == 239
    assert second.song.url == "https://www.youtube.com/watch?v=ko70cExuzZM"


def test_artist_songs(entry: Moounit):
    entry.replay_ignore(
        request=BatchHttpRequest(),
        ignore_fields=[
            "headers.X-Goog-Visitor-Id",
            "body",
            "headers.user-agent",
        ],
    )
    entry.replay_ignore(
        request=SetCacheRequest(),
        ignore_fields=["data"],
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
    assert first.song.id == "app.moosync.youtube.js:youtube:9kznlAwE-8o"
    assert first.song.title == "Mockingbird"
    assert first.song.duration.seconds == 251
    assert first.song.url == "https://www.youtube.com/watch?v=9kznlAwE-8o"
    assert len(first.artists) == 1
    assert (
        first.artists[0].artist_id
        == "app.moosync.youtube.js:youtube-artist:UCedvOgsKFzcK3hA5taf3KoQ"
    )
    assert first.artists[0].artist_name == "Eminem"

    second = songs[1]
    assert second.song.id == "app.moosync.youtube.js:youtube:RnkShwdXfyc"
    assert second.song.title == "Love The Way You Lie (feat. Rihanna)"
    assert second.song.duration.seconds == 264
    assert second.song.url == "https://www.youtube.com/watch?v=RnkShwdXfyc"
    assert len(second.artists) == 1
    assert (
        second.artists[0].artist_id
        == "app.moosync.youtube.js:youtube-artist:UCedvOgsKFzcK3hA5taf3KoQ"
    )
    assert second.artists[0].artist_name == "Eminem"


def test_album_songs(entry: Moounit):
    entry.replay_ignore(
        request=BatchHttpRequest(),
        ignore_fields=[
            "headers.X-Goog-Visitor-Id",
            "body",
            "headers.user-agent",
        ],
    )
    entry.replay_ignore(
        request=SetCacheRequest(),
        ignore_fields=["data"],
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
    assert first.song.id == "app.moosync.youtube.js:youtube:e_CrkgWHEB0"
    assert first.song.title == "Nevermind, Not Nirvana"
    assert first.song.duration.seconds == 147
    assert first.song.url == "https://www.youtube.com/watch?v=e_CrkgWHEB0"


def test_recommendations(entry: Moounit):
    entry.replay_ignore(
        request=BatchHttpRequest(),
        ignore_fields=[
            "headers.X-Goog-Visitor-Id",
            "body",
            "headers.user-agent",
        ],
    )
    entry.replay_ignore(
        request=SetCacheRequest(),
        ignore_fields=["data"],
    )
    ret = entry.send_command(
        ExtensionCommand(requested_recommendations=RequestedRecommendationsRequest())
    )
    songs = ret.requested_recommendations.songs
    assert len(songs) == 68

    first = songs[0]
    assert first.song.id == "app.moosync.youtube.js:youtube:MPREb_3pF7GwaQiMk"
    assert first.song.title == "Helluva Boss Season Three, Part 1 (Deluxe)"
    assert first.song.url == "https://www.youtube.com/watch?v=MPREb_3pF7GwaQiMk"
    assert len(first.artists) == 1
    assert (
        first.artists[0].artist_id
        == "app.moosync.youtube.js:youtube-artist:UCDl4z9JAB6B8zf84A27mv_Q"
    )
    assert first.artists[0].artist_name == "Helluva Boss"


def test_oauth_flow(entry: Moounit):
    entry.replay_ignore(
        request=BatchHttpRequest(),
        ignore_fields=[
            "headers.X-Goog-Visitor-Id",
            "body",
            "headers.user-agent",
        ],
    )
    entry.replay_ignore(
        request=SetCacheRequest(),
        ignore_fields=["data"],
    )
    entry.replay_ignore(
        request=MainCommand(),
        ignore_fields=["set_secure.data.value.string_value"],
    )

    pre_login_account = entry.accounts.get("youtube")
    assert pre_login_account is not None
    assert pre_login_account.id == "youtube"
    assert pre_login_account.name == "YouTube"
    assert pre_login_account.package_name == "app.moosync.youtube.js"
    assert pre_login_account.bg_color == "#FF0000"
    assert pre_login_account.logged_in is False

    ret = entry.send_command(
        ExtensionCommand(
            perform_account_login=PerformAccountLoginRequest(login_status=True)
        )
    )
    assert ret.perform_account_login.url == "https://www.google.com/device"
    assert ret.perform_account_login.device_code == "ZXS-XFK-XTHK"

    cb_ret = entry.send_command(
        ExtensionCommand(oauth_callback=OauthCallbackRequest(callback_uri="done"))
    )
    assert cb_ret.oauth_callback is not None

    account = entry.accounts.get("youtube")
    assert account is not None
    assert account.id == "youtube"
    assert account.name == "YouTube"
    assert account.package_name == "app.moosync.youtube.js"
    assert account.bg_color == "#FF0000"
    assert account.logged_in is True
    assert account.username == "Sahil Gupte"


def test_requested_playlists(entry: Moounit):
    entry.replay_ignore(
        request=BatchHttpRequest(),
        ignore_fields=[
            "headers.X-Goog-Visitor-Id",
            "body",
            "headers.user-agent",
        ],
    )
    entry.replay_ignore(
        request=SetCacheRequest(),
        ignore_fields=["data"],
    )
    entry.replay_ignore(
        request=MainCommand(),
        ignore_fields=["set_secure.data.value.string_value"],
    )
    entry.expect_command(
        MainCommand(get_cache=GetCacheRequest(key="user_playlists")),
        MainCommandResponse(get_cache=GetCacheResponse()),
    )
    entry.expect_command(
        MainCommand(set_cache=SetCacheRequest(key="user_playlists")),
        MainCommandResponse(set_cache=SetCacheResponse(success=True)),
    )

    pre_login_account = entry.accounts.get("youtube")
    assert pre_login_account is not None
    assert pre_login_account.logged_in is False

    ret = entry.send_command(
        ExtensionCommand(
            perform_account_login=PerformAccountLoginRequest(login_status=True)
        )
    )
    assert ret.perform_account_login.url == "https://www.google.com/device"
    assert ret.perform_account_login.device_code == "ZXS-XFK-XTHK"

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
        == "app.moosync.youtube.js:youtube-playlist:PLDIoUOhQQPlU2DoseElwLlKXmV9YSEnKI"
    )
    assert first.playlist_name == "Liked Music"
    assert first.playlist_song_count == 98
    assert (
        first.playlist_coverpath == "https://i.ytimg.com/vi/EFqvVSqh6R8/hqdefault.jpg"
    )

    second = playlists[1]
    assert (
        second.playlist_id
        == "app.moosync.youtube.js:youtube-playlist:PLO0BbVUKhzndecKXOAEm3I76w46RNLkgK"
    )
    assert second.playlist_name == "kek"
    assert second.playlist_song_count == 25
    assert (
        second.playlist_coverpath == "https://i.ytimg.com/vi/R_7eZYhkcOg/hqdefault.jpg"
    )


def test_playback_details(entry: Moounit):
    entry.replay_ignore(
        request=BatchHttpRequest(),
        ignore_fields=[
            "headers.X-Goog-Visitor-Id",
            "body",
            "headers.user-agent",
        ],
    )
    entry.replay_ignore(
        request=SetCacheRequest(),
        ignore_fields=["data"],
    )
    ret = entry.send_command(
        ExtensionCommand(
            playback_details_requested=PlaybackDetailsRequestedRequest(
                song=Song(song=InnerSong(id="app.moosync.youtube_js:dQw4w9WgXcQ"))
            )
        )
    )
    playback = ret.playback_details_requested
    assert playback.url.startswith("https://")
    assert "googlevideo.com" in playback.url
    assert playback.url.endswith(".m3u8")
    assert playback.duration.seconds == 213
    assert playback.HasField("expires_at")
    assert playback.expires_at.seconds > 0


def test_playback_details_cache_hit(entry: Moounit):
    cached_playback = PlaybackDetailsRequestedResponse(
        url="https://cached.googlevideo.com/videoplayback/test.m3u8",
        duration=Duration(seconds=213),
    )
    entry.expect_command(
        command=MainCommand(get_cache=GetCacheRequest(key="playback:cached_vid1")),
        response=MainCommandResponse(
            get_cache=GetCacheResponse(data=cached_playback.SerializeToString())
        ),
    )

    ret = entry.send_command(
        ExtensionCommand(
            playback_details_requested=PlaybackDetailsRequestedRequest(
                song=Song(song=InnerSong(id="app.moosync.youtube_js:cached_vid1"))
            )
        )
    )
    playback = ret.playback_details_requested
    assert playback.url == "https://cached.googlevideo.com/videoplayback/test.m3u8"
    assert playback.duration.seconds == 213
