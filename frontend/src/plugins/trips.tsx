// Trip Planner Plugin — React UI component with event list and map markers

import { useState, useEffect } from 'react'
import { callPluginEndpoint, type Node } from '../api/client'

export default function TripsUI() {
  const [trips, setTrips] = useState<Node[]>([])
  const [events, setEvents] = useState<Node[]>([])
  const [mapEvents, setMapEvents] = useState<any[]>([])
  const [selectedTrip, setSelectedTrip] = useState<string | null>(null)
  const [loading, setLoading] = useState(true)

  const [newTripTitle, setNewTripTitle] = useState('')
  const [newEventTitle, setNewEventTitle] = useState('')
  const [newEventLat, setNewEventLat] = useState('')
  const [newEventLng, setNewEventLng] = useState('')
  const [newEventLoc, setNewEventLoc] = useState('')
  const [newEventTime, setNewEventTime] = useState('')

  const PLUGIN_ID = 'com.panorama.trips'

  const fetchData = async () => {
    setLoading(true)
    try {
      const tRes = await callPluginEndpoint(PLUGIN_ID, 'trips')
      setTrips(await tRes.json())
      const eRes = await callPluginEndpoint(PLUGIN_ID, 'events', 'GET')
      setEvents(await eRes.json())
      const mRes = await callPluginEndpoint(PLUGIN_ID, 'events/map')
      setMapEvents(await mRes.json())
    } catch (e) {}
    setLoading(false)
  }

  useEffect(() => { fetchData() }, [])

  const createTrip = async () => {
    if (!newTripTitle) return
    await callPluginEndpoint(PLUGIN_ID, 'trips', 'POST', {
      title: newTripTitle,
      start_date: new Date().toISOString(),
    })
    setNewTripTitle('')
    fetchData()
  }

  const createEvent = async () => {
    if (!newEventTitle || !selectedTrip) return
    await callPluginEndpoint(PLUGIN_ID, 'events', 'POST', {
      title: newEventTitle,
      trip_id: selectedTrip,
      start_time: newEventTime || new Date().toISOString(),
      latitude: parseFloat(newEventLat) || undefined,
      longitude: parseFloat(newEventLng) || undefined,
      location_name: newEventLoc || undefined,
    })
    setNewEventTitle(''); setNewEventLat(''); setNewEventLng(''); setNewEventLoc(''); setNewEventTime('')
    fetchData()
  }

  const filteredEvents = selectedTrip
    ? events.filter(e => e.fields['trips:trip_id']?.value === selectedTrip)
    : events

  return (
    <div style={{ maxWidth: 900, margin: '0 auto' }}>
      <h2 style={{ marginBottom: 16 }}> Trip Planner</h2>

      {/* Create trip */}
      <div className="card flex-row" style={{ marginBottom: 16 }}>
        <input placeholder="Trip name" value={newTripTitle} onChange={e => setNewTripTitle(e.target.value)} style={{ flex: 1 }} />
        <button className="primary" onClick={createTrip}>+ Trip</button>
      </div>

      {/* Trip selector */}
      <div className="flex-row" style={{ gap: 8, marginBottom: 16, flexWrap: 'wrap' }}>
        {trips.map(t => (
          <button key={t.id}
            className={selectedTrip === t.id ? 'primary' : ''}
            onClick={() => setSelectedTrip(selectedTrip === t.id ? null : t.id)}
          >
            {t.fields['system:node_title']?.value || 'Unnamed'}
          </button>
        ))}
      </div>

      {/* Add event */}
      {selectedTrip && (
        <div className="card" style={{ marginBottom: 16 }}>
          <h4>Add Event to {trips.find(t => t.id === selectedTrip)?.fields['system:node_title']?.value}</h4>
          <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 8, marginTop: 8 }}>
            <input placeholder="Event name" value={newEventTitle} onChange={e => setNewEventTitle(e.target.value)} />
            <input type="datetime-local" value={newEventTime} onChange={e => setNewEventTime(e.target.value)} />
            <input placeholder="Latitude" value={newEventLat} onChange={e => setNewEventLat(e.target.value)} />
            <input placeholder="Longitude" value={newEventLng} onChange={e => setNewEventLng(e.target.value)} />
            <input placeholder="Location name" value={newEventLoc} onChange={e => setNewEventLoc(e.target.value)} style={{ gridColumn: 'span 2' }} />
          </div>
          <button className="primary" onClick={createEvent} style={{ marginTop: 8 }}>+ Event</button>
        </div>
      )}

      {/* Event list + map */}
      <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 16 }}>
        <div>
          <h4>Events {selectedTrip ? '(filtered)' : '(all)'}</h4>
          {loading ? <p>Loading...</p> : (
            <div style={{ display: 'flex', flexDirection: 'column', gap: 8, marginTop: 8 }}>
              {filteredEvents.map(e => (
                <div key={e.id} className="card">
                  <strong>{e.fields['system:node_title']?.value || 'Event'}</strong>
                  <p className="text-muted" style={{ fontSize: 13 }}>
                    {e.fields['trips:location_name']?.value || 'No location'} · {e.fields['system:node_start_time']?.value?.slice(0, 10)}
                  </p>
                </div>
              ))}
              {filteredEvents.length === 0 && <p className="text-muted">No events. Create a trip first, then add events.</p>}
            </div>
          )}
        </div>

        <div>
          <h4>Map Locations</h4>
          <div className="card" style={{ minHeight: 300, marginTop: 8, background: 'var(--bg)' }}>
            {mapEvents.length === 0 ? (
              <p className="text-muted" style={{ padding: 16 }}>No geo-tagged events. Add latitude/longitude to events to see them here.</p>
            ) : (
              <div style={{ display: 'flex', flexDirection: 'column', gap: 4, padding: 8 }}>
                {mapEvents.map((ev: any) => (
                  <div key={ev.id} className="flex-row" style={{
                    padding: '8px', borderRadius: 6, background: 'var(--bg-card)',
                    justifyContent: 'space-between',
                  }}>
                    <span>{ev.title || 'Event'}</span>
                    <span className="text-muted" style={{ fontSize: 12 }}>
                      {ev.latitude?.toFixed(4)}, {ev.longitude?.toFixed(4)} — {ev.location}
                    </span>
                  </div>
                ))}
              </div>
            )}
          </div>
        </div>
      </div>
    </div>
  )
}
