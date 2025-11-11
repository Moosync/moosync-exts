from moosync_edk import register_extension, Extension, ProviderScopes, SearchReturnType, Song
from typing import TypedDict, Optional, List, Any, cast

from moosync_edk.custom_types import Album, CustomRequestReturnType, SongsWithPageTokenReturnType, Playlist, Artist

from youtube_dl import ytdl

actions_list = {}

class Thumbnail(TypedDict, total=False):
    url: str
    height: int
    width: int

class YtdlEntry(TypedDict, total=False):
    _type: str
    ie_key: str
    id: str
    url: str
    title: str
    description: Optional[str]
    duration: float
    channel_id: str
    channel: str
    channel_url: str
    uploader: str
    uploader_id: str
    uploader_url: str
    thumbnails: List[Thumbnail]
    timestamp: Optional[Any]
    release_timestamp: Optional[Any]
    availability: Optional[Any]
    view_count: Optional[int]
    live_status: Optional[Any]
    channel_is_verified: Optional[bool]
    
class Format(TypedDict, total=False):
    format_id: str
    format_note: Optional[str]
    ext: str  # file extension
    vcodec: str  # video codec
    video_ext: Optional[str]  # video file extension
    audio_ext: Optional[str]  # audio file extension
    acodec: Optional[str]  # audio codec
    abr: Optional[float]  # audio bitrate
    vbr: Optional[float]  # video bitrate
    filesize: Optional[int]  # file size in bytes
    url: str

class YTDLSong(TypedDict, total=False):
    id: str
    title: str
    formats: List[Format]
    duration: int  # Duration in seconds
    webpage_url: str
    
def get_best_audio_format(song: YTDLSong) -> Optional[Format]:
    # Filter out formats with no audio (where vcodec is 'none' means audio-only format)

    audio_formats = [f for f in song.get('formats', []) if f.get('video_ext', 'none') == 'none' and f.get('audio_ext', 'none') != 'none']
    if not audio_formats:
        return None
        
    # Sort by audio bitrate (abr) in descending order
    # Some formats might not have abr, so use 0 as default
    # return audio_formats
    print(audio_formats)
    return max(audio_formats, key=lambda x: x.get('abr', 0) or 0)
    
def to_song(ytdl_song: YtdlEntry) -> Song:
    thumbnails = ytdl_song.get("thumbnails") or []
    high_url = ""
    low_url = ""
    if thumbnails:
        low_url = thumbnails[0].get("url", None) or None
        high_url = thumbnails[-1].get("url", None) or None
        
    id = ytdl_song.get("id", "")

    return Song(
        _id = id,
        title=ytdl_song.get("title", ""),
        artists=[Artist(ytdl_song.get("uploader_id", ""), ytdl_song.get("uploader", ""))],
        song_coverPath_high=high_url,
        song_coverPath_low=low_url,
        duration=int(ytdl_song.get("duration", 0)),
        playbackUrl=f"extension://moosync.youtubedl/{id}",
        type="URL",
    )
    
def to_playlist(ytdl_playlist: YtdlEntry) -> Playlist:
    thumbnails = ytdl_playlist.get("thumbnails") or []
    return Playlist(
        playlist_id=ytdl_playlist.get("id", ""),
        playlist_name=ytdl_playlist.get("title", ""),
        playlist_coverPath=thumbnails[-1].get("url", None),
    )
    
def to_artist(ytdl_artist: YtdlEntry) -> Artist:
    thumbnails = ytdl_artist.get("thumbnails") or []
    return Artist(
        artist_id=ytdl_artist.get("id", ""),
        artist_name=ytdl_artist.get("title", ""),
        artist_coverPath=thumbnails[-1].get("url", None),
    )
        
class YoutubeDlExtension(Extension):
    def get_provider_scopes(self) -> List[ProviderScopes]:
        return ["search", "playbackDetails", "playlistSongs", "artistSongs", "albumSongs"]
    
    def get_album_songs(self, album: Album, token: str | None = None) -> SongsWithPageTokenReturnType:
        if not album.album_id:
            return SongsWithPageTokenReturnType([], None)
        
        try:
            search_query = f"https://www.youtube.com/playlist?list={album.album_id}"
            playlist_result = ytdl.extract_info(search_query, download=False, process=False)
            
            if not playlist_result:
                return SongsWithPageTokenReturnType([], None)
                
            entries = playlist_result.get("entries", [])
            if not entries:
                return SongsWithPageTokenReturnType([], None)
                
            valid_entries = [cast(YtdlEntry, e) for e in entries if e is not None]
            songs = [to_song(entry) for entry in valid_entries]
            
            return SongsWithPageTokenReturnType(songs, None)
        except Exception as e:
            print("Exception in get_album_songs:", e)
            return SongsWithPageTokenReturnType([], None)
    
    def get_artist_songs(self, artist: Artist, token: str | None = None) -> SongsWithPageTokenReturnType:
        print("Got artist", artist)
        if not artist.artist_id:
            return SongsWithPageTokenReturnType([], None)
        
        try:
            search_query = f"https://www.youtube.com/channel/{artist.artist_id}/videos"
            channel_result = ytdl.extract_info(search_query, download=False, process=False)
            print("channel result", channel_result)
            
            if not channel_result:
                return SongsWithPageTokenReturnType([], None)
                
            entries = channel_result.get("entries", [])
            if not entries:
                return SongsWithPageTokenReturnType([], None)
                
            valid_entries = [cast(YtdlEntry, e) for e in entries if e is not None]
            songs = [to_song(entry) for entry in valid_entries]
            
            return SongsWithPageTokenReturnType(songs, None)
        except Exception as e:
            print("Exception in get_artist_songs:", e)
            return SongsWithPageTokenReturnType([], None)
    
    def get_playlist_content(self, id: str, token: str | None = None) -> SongsWithPageTokenReturnType:
        try:
            search_query = f"https://www.youtube.com/playlist?list={id}"
            playlist_result = ytdl.extract_info(search_query, download=False, process=False)
            
            if not playlist_result:
                return SongsWithPageTokenReturnType([], None)
                
            entries = playlist_result.get("entries", [])
            if not entries:
                return SongsWithPageTokenReturnType([], None)
                
            valid_entries = [cast(YtdlEntry, e) for e in entries if e is not None]
            songs = [to_song(entry) for entry in valid_entries]
            
            return SongsWithPageTokenReturnType(songs, None)
        except Exception as e:
            print("Exception in get_playlist_content:", e)
            return SongsWithPageTokenReturnType([], None)
    
    def get_search(self, term: str) -> SearchReturnType:
        songs: List[Song] = []
        artists: List[Artist] = []
        albums: List[Album] = []
        playlists: List[Playlist] = []

        search_query_video = f"https://www.youtube.com/results?search_query={term}&sp=EgIQAQ%253D%253D"
        search_query_channel = f"https://www.youtube.com/results?search_query={term}&sp=EgIQAg%253D%253D"
        search_query_playlist = f"https://www.youtube.com/results?search_query={term}&sp=EgIQAw%253D%253D"
        
        search_queries = [search_query_video, search_query_channel, search_query_playlist]
        
        for query in search_queries:
            try:
                search_result = ytdl.extract_info(query, download=False, process=False)
                if not search_result:
                    continue
                
                entries: List[YtdlEntry] = search_result.get('entries', [])
                if not entries:
                    continue
                
                print(f"Processing '{query}'")
                i = 0
                for entry in entries:
                    url = entry.get('url', '')
                    if not entry.get('id'):
                        continue

                    if url and url.startswith("https://www.youtube.com/watch?v=") and entry.get('duration', None) is not None:
                        songs.append(to_song(entry))
                    elif url and url.startswith("https://www.youtube.com/playlist?list="):
                        playlists.append(to_playlist(entry))
                    elif url and url.startswith("https://www.youtube.com/channel/"):
                        artists.append(to_artist(entry))
                    
                    i += 1
                    if i >= 10:
                        break
            except Exception as e:
                print(f"Exception in get_search for query '{query}':", e)
            
        return SearchReturnType(songs, artists, playlists, albums, [])
    
    def handle_custom_request(self, url: str) -> CustomRequestReturnType:
        print("Handling custom request for URL:", url)
        if url.startswith("extension://moosync.youtubedl/"):
            song_id = url.replace("extension://moosync.youtubedl/", "")
            ytdl_song = cast(YTDLSong | None, ytdl.extract_info(song_id, download=False))
            if ytdl_song is not None:
                best_audio_format = get_best_audio_format(ytdl_song)
                if best_audio_format is not None:
                    return CustomRequestReturnType(None, None, best_audio_format.get('url'))
                
        return CustomRequestReturnType(None, None, None)

def init():
    register_extension(YoutubeDlExtension())
    print("initialized YTDL extension")