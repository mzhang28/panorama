// Subsonic Music Plugin — React UI component

import { useState, useEffect } from 'react'
import { callPluginEndpoint, type Node } from '../api/client'

export default function SubsonicUI() {
  const [artists, setArtists] = useState<any[]>([])
  const [albums, setAlbums] = useState<any[]>([])
  const [loading, setLoading] = useState(true)
  const [pingResult, setPingResult] = useState<any>(null)
  const [uploading, setUploading] = useState(false)

  const PLUGIN_ID = 'com.panorama.subsonic'

  const fetchLibrary = async () => {
    setLoading(true)
    try {
      const ping = await callPluginEndpoint(PLUGIN_ID, 'rest/ping')
      setPingResult(await ping.json())

      const aRes = await callPluginEndpoint(PLUGIN_ID, 'rest/getArtists')
      const aData = await aRes.json()
      setArtists(aData['subsonic-response']?.artists?.index?.[0]?.artist || [])

      const alRes = await callPluginEndpoint(PLUGIN_ID, 'rest/getAlbumList2')
      const alData = await alRes.json()
      setAlbums(alData['subsonic-response']?.albumList2?.album || [])
    } catch (e) {}
    setLoading(false)
  }

  useEffect(() => { fetchLibrary() }, [])

  const handleUpload = async (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0]
    if (!file) return
    setUploading(true)
    try {
      const url = `/plugin/${PLUGIN_ID}/upload?filename=${encodeURIComponent(file.name)}&title=${encodeURIComponent(file.name.replace(/\.[^.]+$/, ''))}`
      await fetch(`http://localhost:3000${url}`, { method: 'POST', body: file })
      fetchLibrary()
    } catch (e) {}
    setUploading(false)
  }

  return (
    <div style={{ maxWidth: 800, margin: '0 auto' }}>
      <h2 style={{ marginBottom: 16 }}> Music Library</h2>

      {pingResult && (
        <div className="card" style={{ marginBottom: 16 }}>
          <div className="flex-row" style={{ justifyContent: 'space-between' }}>
            <span>Server: <strong>{pingResult['subsonic-response']?.serverVersion || 'unknown'}</strong></span>
            <span className="text-muted">Type: {pingResult['subsonic-response']?.type || 'unknown'}</span>
          </div>
        </div>
      )}

      <div className="card" style={{ marginBottom: 16 }}>
        <h4>Upload Music</h4>
        <input type="file" accept="audio/*" onChange={handleUpload} disabled={uploading} style={{ marginTop: 8 }} />
        {uploading && <p className="text-muted" style={{ marginTop: 4 }}>Uploading...</p>}
      </div>

      {loading ? <p>Loading...</p> : (
        <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 16 }}>
          <div>
            <h4>Artists ({artists.length})</h4>
            <div style={{ display: 'flex', flexDirection: 'column', gap: 4, marginTop: 8 }}>
              {artists.map((a: any) => (
                <div key={a.id} className="card" style={{ padding: '8px 12px' }}>
                  <strong>{a.name}</strong>
                </div>
              ))}
              {artists.length === 0 && <p className="text-muted">No artists</p>}
            </div>
          </div>

          <div>
            <h4>Albums ({albums.length})</h4>
            <div style={{ display: 'flex', flexDirection: 'column', gap: 4, marginTop: 8 }}>
              {albums.map((a: any) => (
                <div key={a.id} className="card" style={{ padding: '8px 12px' }}>
                  <strong>{a.name}</strong>
                  {a.artistId && <span className="text-muted" style={{ marginLeft: 8 }}>by artist {a.artistId}</span>}
                </div>
              ))}
              {albums.length === 0 && <p className="text-muted">No albums</p>}
            </div>
          </div>
        </div>
      )}
    </div>
  )
}
