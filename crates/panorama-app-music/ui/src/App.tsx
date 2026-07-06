// Music Library Plugin — React UI component.
// Loaded by the Panorama host via Module Federation at runtime.

import { useState, useEffect } from 'react'

// ── Minimal API helper (self-contained; no dependency on host client) ────────

async function callPluginEndpoint(
  pluginId: string,
  endpoint: string,
  method = 'GET',
  body?: unknown,
): Promise<Response> {
  const opts: RequestInit = {
    method,
    headers: body ? { 'Content-Type': 'application/json' } : {},
    body: body ? JSON.stringify(body) : undefined,
  }
  return fetch(`/plugin/${pluginId}/${endpoint}`, opts)
}

// ── Types ───────────────────────────────────────────────────────────────────

interface MusicAppProps {
  pluginId: string
}

// ── Component ───────────────────────────────────────────────────────────────

export default function MusicApp({ pluginId }: MusicAppProps) {
  const PLUGIN_ID = pluginId || 'io.mzhang.panorama.music'
  const [artists, setArtists] = useState<any[]>([])
  const [albums, setAlbums] = useState<any[]>([])
  const [loading, setLoading] = useState(true)
  const [pingResult, setPingResult] = useState<any>(null)
  const [uploading, setUploading] = useState(false)

  const fetchLibrary = async () => {
    setLoading(true)
    try {
      const ping = await callPluginEndpoint(PLUGIN_ID, 'rest/ping')
      setPingResult(await ping.json())

      const aRes = await callPluginEndpoint(PLUGIN_ID, 'rest/getArtists')
      const aData = await aRes.json()
      setArtists(
        aData['music-response']?.artists?.index?.[0]?.artist || [],
      )

      const alRes = await callPluginEndpoint(
        PLUGIN_ID,
        'rest/getAlbumList2',
      )
      const alData = await alRes.json()
      setAlbums(alData['music-response']?.albumList2?.album || [])
    } catch (_e) {}
    setLoading(false)
  }

  useEffect(() => {
    fetchLibrary()
  }, [])

  const handleUpload = async (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0]
    if (!file) return
    setUploading(true)
    try {
      const url = `/plugin/${PLUGIN_ID}/upload?filename=${encodeURIComponent(file.name)}&title=${encodeURIComponent(file.name.replace(/\.[^.]+$/, ''))}`
      await fetch(url, { method: 'POST', body: file })
      fetchLibrary()
    } catch (_e) {}
    setUploading(false)
  }

  return (
    <div style={{ maxWidth: 800, margin: '0 auto' }}>
      <h2 style={{ marginBottom: 16 }}>Music Library</h2>

      {pingResult && (
        <div className="card" style={{ marginBottom: 16 }}>
          <div
            className="flex-row"
            style={{ justifyContent: 'space-between' }}
          >
            <span>
              Server:{' '}
              <strong>
                {pingResult['music-response']?.serverVersion ||
                  'unknown'}
              </strong>
            </span>
            <span className="text-muted">
              Type: {pingResult['music-response']?.type || 'unknown'}
            </span>
          </div>
        </div>
      )}

      <div className="card" style={{ marginBottom: 16 }}>
        <h4>Upload Music</h4>
        <input
          type="file"
          accept="audio/*"
          onChange={handleUpload}
          disabled={uploading}
          style={{ marginTop: 8 }}
        />
        {uploading && (
          <p className="text-muted" style={{ marginTop: 4 }}>
            Uploading...
          </p>
        )}
      </div>

      {loading ? (
        <p>Loading...</p>
      ) : (
        <div
          style={{
            display: 'grid',
            gridTemplateColumns: '1fr 1fr',
            gap: 16,
          }}
        >
          <div>
            <h4>Artists ({artists.length})</h4>
            <div
              style={{
                display: 'flex',
                flexDirection: 'column',
                gap: 4,
                marginTop: 8,
              }}
            >
              {artists.map((a: any) => (
                <div
                  key={a.id}
                  className="card"
                  style={{ padding: '8px 12px' }}
                >
                  <strong>{a.name}</strong>
                </div>
              ))}
              {artists.length === 0 && (
                <p className="text-muted">No artists</p>
              )}
            </div>
          </div>

          <div>
            <h4>Albums ({albums.length})</h4>
            <div
              style={{
                display: 'flex',
                flexDirection: 'column',
                gap: 4,
                marginTop: 8,
              }}
            >
              {albums.map((a: any) => (
                <div
                  key={a.id}
                  className="card"
                  style={{ padding: '8px 12px' }}
                >
                  <strong>{a.name}</strong>
                  {a.artistId && (
                    <span className="text-muted" style={{ marginLeft: 8 }}>
                      by artist {a.artistId}
                    </span>
                  )}
                </div>
              ))}
              {albums.length === 0 && (
                <p className="text-muted">No albums</p>
              )}
            </div>
          </div>
        </div>
      )}
    </div>
  )
}
