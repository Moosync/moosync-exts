import { Artist, Exports, Playlist, Song, api } from '@moosync/edk'
import { SoundcloudApi } from './soundcloudApi'

class SoundCloudExtension {
  private soundcloudApi = new SoundcloudApi(this.updateKey.bind(this))

  private updateKey(key: string) {
    api.setSecure({ key: 'apiKey', value: key })
  }

  private fetchPreferences() {
    const key = api.getSecure<string>({ key: 'apiKey' })
    this.soundcloudApi.generateKey(key)
  }

  registerListeners() {
    api.on('getProviderScopes', () => {
      return [
        'search',
        'artistSongs',
        'albumSongs',
        'playlistFromUrl',
        'playbackDetails',
        'songFromUrl',
        'playlistSongs'
      ]
    })

    api.on('getSearch', async (term) => {
      const songs = await this.soundcloudApi.searchSongs(term, false)
      const artists = await this.soundcloudApi.searchArtist(term, false)
      const playlists = await this.soundcloudApi.searchPlaylists(term, false)
      return {
        songs,
        artists,
        albums: [],
        playlists,
        genres: []
      }
    })

    api.on('getArtistSongs', async (artist: Artist) => {
      const soundcloudArtist = (await this.soundcloudApi.searchArtist(artist.artist_name, false))[0]
      if (soundcloudArtist) {
        const artistId = soundcloudArtist.artist_id.replace('soundcloud:users:', '')
        const songs = await this.soundcloudApi.getArtistSongs(artistId, false)
        return { songs }
      }

      return {
        songs: []
      }
    })

    api.on('getSongFromUrl', async (url) => {
      const song = (await this.soundcloudApi.parseUrl(url, false)) as unknown as Song
      if (song) return { song }
    })

    api.on('getPlaylistFromUrl', async (url) => {
      const data = (await this.soundcloudApi.parseUrl(url, false)) as unknown as { songs: Song[]; playlist: Playlist }
      if (data) return { songs: data.songs, playlist: data.playlist }
    })

    api.on('getPlaybackDetails', async (song) => {
      if (song.url) {
        const data = await this.soundcloudApi.fetchFromStreamURL(song.url)
        return { duration: song.duration, url: data.url }
      }
    })

    api.on('handleCustomRequest', async (url) => {
      try {
        console.log('got custom request', url)
        const redirectUrl = await this.soundcloudApi.getSongStreamById(new URL(url).pathname.substring(1), false)
        console.log('got redirect url', redirectUrl)
        return { redirectUrl }
      } catch (e) {
        console.error(e, url)
      }
    })

    api.on('getPlaylistContent', async (id) => {
      const playlistId = id.replace('moosync.soundcloud:', '')
      const songs = await this.soundcloudApi.getPlaylistSongs(playlistId, false)
      return {
        songs
      }
    })

    api.on('getSongFromId', async (id) => {
      const song = await this.soundcloudApi.getSongById(parseInt(id), false)
      return {
        song
      }
    })
  }
}

export function entry() {
  console.log('Initializing soundcloud ext')
  const ext = new SoundCloudExtension()
  ext.registerListeners()
  console.log('Initialized soundcloud ext')
}

module.exports = {
  ...module.exports,
  ...require('@moosync/edk').Exports
}
