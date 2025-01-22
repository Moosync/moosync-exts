from moosync_edk import register_extension, Extension, ContextMenuReturnType, ProviderScopes, http_request
from typing import List
import uuid

from youtube_dl import ytdl

actions_list = {}
class YoutubeDlExtension(Extension):
    def get_provider_scopes(self) -> List[ProviderScopes]:
        return ["songContextMenu"]

    def on_context_menu_action(self, action):
        urls = actions_list[action]
        ytdl.download(urls)
        return
    
    def get_song_context_menu(self, songs) -> List[ContextMenuReturnType]:
        global actions_list

        action_id = uuid.uuid1()

        ret = []
        for song in songs:
            if song.type == "YOUTUBE":
                ret.append(f"https://www.youtube.com/watch?v={song.playbackUrl}")
        actions_list[str(action_id)] = ret
        
        return [ContextMenuReturnType("Download song", "", str(action_id))]

def init():
    register_extension(YoutubeDlExtension())
    print("initialized YTDL extension")