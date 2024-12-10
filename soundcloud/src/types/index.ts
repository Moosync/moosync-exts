export interface Playlists {
  artwork_url: any
  created_at: string
  description: any
  duration: number
  embeddable_by: string
  genre: string
  id: number
  kind: string
  label_name: any
  last_modified: string
  license: string
  likes_count: number
  managed_by_feeds: boolean
  permalink: string
  permalink_url: string
  public: boolean
  purchase_title: any
  purchase_url: any
  release_date: any
  reposts_count: number
  secret_token: any
  sharing: string
  tag_list: string
  title: string
  uri: string
  user_id: number
  set_type: string
  is_album: boolean
  published_at: string
  display_date: string
  user: User
  tracks: Tracks[]
  track_count: number
}

interface User {
  avatar_url: string
  city: any
  comments_count: number
  country_code: any
  created_at: string
  creator_subscriptions: CreatorSubscription[]
  creator_subscription: CreatorSubscription2
  description: any
  followers_count: number
  followings_count: number
  first_name: string
  full_name: string
  groups_count: number
  id: number
  kind: string
  last_modified: string
  last_name: string
  likes_count: number
  playlist_likes_count: number
  permalink: string
  permalink_url: string
  playlist_count: number
  reposts_count: any
  track_count: number
  uri: string
  urn: string
  username: string
  verified: boolean
  visuals: any
  badges: Badges
  station_urn: string
  station_permalink: string
}

interface CreatorSubscription {
  product: Product
}

interface Product {
  id: string
}

interface CreatorSubscription2 {
  product: Product2
}

interface Product2 {
  id: string
}

interface Badges {
  pro: boolean
  pro_unlimited: boolean
  verified: boolean
}

interface PublisherMetadata {
  id: number
  urn: string
}

interface Media {
  transcodings: Transcoding[]
}

interface Transcoding {
  url: string
  preset: string
  duration: number
  snipped: boolean
  format: Format
  quality: string
}

interface Format {
  protocol: string
  mime_type: string
}

interface User2 {
  avatar_url: string
  first_name: string
  followers_count: number
  full_name: string
  id: number
  kind: string
  last_modified: string
  last_name: string
  permalink: string
  permalink_url: string
  uri: string
  urn: string
  username: string
  verified: boolean
  city: string
  country_code: string
  badges: Badges2
  station_urn: string
  station_permalink: string
}

interface Badges2 {
  pro: boolean
  pro_unlimited: boolean
  verified: boolean
}

export interface UserInfo {
  collection: User[]
}

export interface PlaylistInfo {
  collection: Playlists[]
}

export interface TrackInfo {
  collection: Tracks[]
  next_href: string
}

export interface Tracks {
  artwork_url: any
  caption: any
  commentable: boolean
  comment_count: number
  created_at: string
  description: string
  downloadable: boolean
  download_count: number
  duration: number
  full_duration: number
  embeddable_by: string
  genre: string
  has_downloads_left: boolean
  id: number
  kind: string
  label_name: string
  last_modified: string
  license: string
  likes_count: number
  permalink: string
  permalink_url: string
  playback_count: number
  public: boolean
  publisher_metadata: PublisherMetadata
  purchase_title: any
  purchase_url: any
  release_date: any
  reposts_count: number
  secret_token: any
  sharing: string
  state: string
  streamable: boolean
  tag_list: string
  title: string
  track_format: string
  uri: string
  urn: string
  user_id: number
  visuals: any
  waveform_url: string
  display_date: string
  media: Media
  station_urn: string
  station_permalink: string
  track_authorization: string
  monetization_model: string
  policy: string
  user: User
}
