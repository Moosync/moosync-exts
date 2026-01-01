from yt_dlp import YoutubeDL

ytdl = YoutubeDL({
    'nocheckcertificate': True,
    'verbose': True,
    'outtmpl': "/downloads/ytdl/%(title)s.%(ext)s",
    "paths": {
        "home": "/downloads",
        "temp": "/downloads/temp"
    },
    "cachedir": "/downloads/cache",
    'extractor_args': {
        'youtubejsc-remotecipher': {
            'base_url': 'https://cipher.moosync.app'
        }
    }
}, auto_init="no_verbose_header")


ydl_opts = {
    'format': 'bestaudio/best',  # Prefer best quality audio
    'extractaudio': True,        # Only extract audio
    'noplaylist': True,          # Don't download playlists
    'postprocessors': [{
        'key': 'FFmpegExtractAudio',
        'preferredcodec': 'mp3',  # You can change this to m4a, opus, vorbis, etc.
        'preferredquality': '192'  # Audio quality in kbps
    }],
    "extractor_args": {
        'youtubejsc-remotecipher': {
            'base_url': ['https://cipher.whoretard.uk']
        }
    }
}


with YoutubeDL(ydl_opts) as ydl:
    search_query = f"https://www.youtube.com/results?search_query=imagine dragons evolve&sp=EgIQAQ%253D%253D"
    playlist_result = ytdl.extract_info(search_query, download=False, process=False)
    i = 0
    for entry in playlist_result.get("entries", []):
        i += 1
        if i > 10:
            break
        print(entry)