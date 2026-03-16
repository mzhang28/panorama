import React, { useState, useEffect } from 'react';
import { Activity, Scale, Utensils, Map, Dumbbell } from 'lucide-react';

// API Client
const API_BASE = "panorama-api://fitness";

interface ActivityLog {
  id: string;
  type: 'weight' | 'food' | 'workout' | 'gpx';
  timestamp: string;
  data: any;
}

export default function App() {
  const [activeTab, setActiveTab] = useState<'weight' | 'food' | 'workout' | 'gpx'>('weight');
  const [activities, setActivities] = useState<ActivityLog[]>([]);
  const [loading, setLoading] = useState(false);

  // Forms State
  const [weight, setWeight] = useState('');
  const [foodName, setFoodName] = useState('');
  const [calories, setCalories] = useState('');
  const [workoutType, setWorkoutType] = useState('');
  const [duration, setDuration] = useState('');
  const [metaKey, setMetaKey] = useState('');
  const [metaValue, setMetaValue] = useState('');
  const [metadata, setMetadata] = useState<Record<string, string>>({});
  const [gpxFile, setGpxFile] = useState<File | null>(null);

  useEffect(() => {
    fetchActivities();
  }, []);

  const fetchActivities = async () => {
    try {
      const res = await fetch(`${API_BASE}/get_activities`, { method: "POST" });
      const data = await res.json();
      if (Array.isArray(data)) {
        setActivities(data);
      }
    } catch (e) {
      console.error("Failed to fetch activities", e);
    }
  };

  const handleLog = async (data: any) => {
    setLoading(true);
    try {
      const payload = {
        type: activeTab,
        timestamp: new Date().toISOString(),
        data: data
      };
      await fetch(`${API_BASE}/log_activity`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(payload)
      });
      // Reset forms
      setWeight('');
      setFoodName('');
      setCalories('');
      setWorkoutType('');
      setDuration('');
      setMetadata({});
      setGpxFile(null);
      
      await fetchActivities();
    } catch (e) {
      console.error("Failed to log activity", e);
    } finally {
      setLoading(false);
    }
  };

  const submitWeight = () => handleLog({ weight: parseFloat(weight), unit: 'kg' });
  const submitFood = () => handleLog({ name: foodName, calories: parseInt(calories) });
  const submitWorkout = () => handleLog({ type: workoutType, duration: duration, metadata });
  const submitGpx = () => {
    if (!gpxFile) return;
    const reader = new FileReader();
    reader.onload = (e) => {
      const content = e.target?.result as string;
      handleLog({ filename: gpxFile.name, content: content });
    };
    reader.readAsText(gpxFile);
  };

  const addMetadata = () => {
    if (metaKey && metaValue) {
      setMetadata(prev => ({ ...prev, [metaKey]: metaValue }));
      setMetaKey('');
      setMetaValue('');
    }
  };

  return (
    <div className="min-h-screen bg-gray-50 text-gray-900 font-sans flex flex-col">
      <header className="bg-white border-b px-6 py-4 flex items-center gap-2 sticky top-0 z-10">
        <Activity className="text-blue-600" />
        <h1 className="text-xl font-bold">Fitness Tracker</h1>
      </header>

      <main className="flex-grow p-6 max-w-3xl mx-auto w-full space-y-8">
        
        {/* Input Section */}
        <div className="bg-white rounded-xl shadow-sm border overflow-hidden">
          <div className="flex border-b">
            <button 
              onClick={() => setActiveTab('weight')}
              className={`flex-1 p-3 flex justify-center items-center gap-2 hover:bg-gray-50 transition ${activeTab === 'weight' ? 'border-b-2 border-blue-600 text-blue-600 font-medium' : 'text-gray-600'}`}
            >
              <Scale size={18} /> Weight
            </button>
            <button 
              onClick={() => setActiveTab('food')}
              className={`flex-1 p-3 flex justify-center items-center gap-2 hover:bg-gray-50 transition ${activeTab === 'food' ? 'border-b-2 border-blue-600 text-blue-600 font-medium' : 'text-gray-600'}`}
            >
              <Utensils size={18} /> Food
            </button>
            <button 
              onClick={() => setActiveTab('workout')}
              className={`flex-1 p-3 flex justify-center items-center gap-2 hover:bg-gray-50 transition ${activeTab === 'workout' ? 'border-b-2 border-blue-600 text-blue-600 font-medium' : 'text-gray-600'}`}
            >
              <Dumbbell size={18} /> Workout
            </button>
            <button 
              onClick={() => setActiveTab('gpx')}
              className={`flex-1 p-3 flex justify-center items-center gap-2 hover:bg-gray-50 transition ${activeTab === 'gpx' ? 'border-b-2 border-blue-600 text-blue-600 font-medium' : 'text-gray-600'}`}
            >
              <Map size={18} /> GPX
            </button>
          </div>

          <div className="p-6">
            {activeTab === 'weight' && (
              <div className="space-y-4">
                <div>
                  <label className="block text-sm font-medium text-gray-700 mb-1">Weight (kg)</label>
                  <input 
                    type="number" 
                    value={weight}
                    onChange={e => setWeight(e.target.value)}
                    className="w-full p-2 border rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-blue-500 outline-none"
                    placeholder="e.g. 75.5"
                  />
                </div>
                <button 
                  onClick={submitWeight}
                  disabled={!weight || loading}
                  className="w-full bg-blue-600 text-white py-2 rounded-lg hover:bg-blue-700 disabled:opacity-50 font-medium"
                >
                  Log Weight
                </button>
              </div>
            )}

            {activeTab === 'food' && (
              <div className="space-y-4">
                <div>
                  <label className="block text-sm font-medium text-gray-700 mb-1">Food Item</label>
                  <input 
                    type="text" 
                    value={foodName}
                    onChange={e => setFoodName(e.target.value)}
                    className="w-full p-2 border rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-blue-500 outline-none"
                    placeholder="e.g. Banana"
                  />
                </div>
                <div>
                  <label className="block text-sm font-medium text-gray-700 mb-1">Calories</label>
                  <input 
                    type="number" 
                    value={calories}
                    onChange={e => setCalories(e.target.value)}
                    className="w-full p-2 border rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-blue-500 outline-none"
                    placeholder="e.g. 105"
                  />
                </div>
                <button 
                  onClick={submitFood}
                  disabled={!foodName || !calories || loading}
                  className="w-full bg-blue-600 text-white py-2 rounded-lg hover:bg-blue-700 disabled:opacity-50 font-medium"
                >
                  Log Food
                </button>
              </div>
            )}

            {activeTab === 'workout' && (
              <div className="space-y-4">
                <div className="grid grid-cols-2 gap-4">
                  <div>
                    <label className="block text-sm font-medium text-gray-700 mb-1">Type</label>
                    <input 
                      type="text" 
                      value={workoutType}
                      onChange={e => setWorkoutType(e.target.value)}
                      className="w-full p-2 border rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-blue-500 outline-none"
                      placeholder="e.g. Running"
                    />
                  </div>
                  <div>
                    <label className="block text-sm font-medium text-gray-700 mb-1">Duration</label>
                    <input 
                      type="text" 
                      value={duration}
                      onChange={e => setDuration(e.target.value)}
                      className="w-full p-2 border rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-blue-500 outline-none"
                      placeholder="e.g. 30 mins"
                    />
                  </div>
                </div>

                <div className="border rounded-lg p-3 bg-gray-50">
                  <label className="block text-xs font-semibold text-gray-500 uppercase mb-2">Metadata</label>
                  <div className="flex gap-2 mb-2">
                    <input 
                      placeholder="Key (e.g. distance)" 
                      value={metaKey}
                      onChange={e => setMetaKey(e.target.value)}
                      className="flex-1 p-2 border rounded text-sm"
                    />
                    <input 
                      placeholder="Value (e.g. 5km)" 
                      value={metaValue}
                      onChange={e => setMetaValue(e.target.value)}
                      className="flex-1 p-2 border rounded text-sm"
                    />
                    <button onClick={addMetadata} className="px-3 py-1 bg-gray-200 rounded hover:bg-gray-300">+</button>
                  </div>
                  <div className="space-y-1">
                    {Object.entries(metadata).map(([k, v]) => (
                      <div key={k} className="flex justify-between text-sm bg-white p-2 rounded border">
                        <span className="font-medium text-gray-700">{k}:</span>
                        <span className="text-gray-600">{v}</span>
                      </div>
                    ))}
                  </div>
                </div>

                <button 
                  onClick={submitWorkout}
                  disabled={!workoutType || loading}
                  className="w-full bg-blue-600 text-white py-2 rounded-lg hover:bg-blue-700 disabled:opacity-50 font-medium"
                >
                  Log Workout
                </button>
              </div>
            )}

            {activeTab === 'gpx' && (
              <div className="space-y-4">
                <div className="border-2 border-dashed border-gray-300 rounded-lg p-8 text-center hover:bg-gray-50 transition cursor-pointer relative">
                  <input 
                    type="file" 
                    accept=".gpx"
                    onChange={e => setGpxFile(e.target.files?.[0] || null)}
                    className="absolute inset-0 w-full h-full opacity-0 cursor-pointer"
                  />
                  <div className="space-y-2 pointer-events-none">
                    <Map className="mx-auto text-gray-400" size={32} />
                    <p className="text-sm text-gray-600 font-medium">
                      {gpxFile ? gpxFile.name : "Click to upload GPX file"}
                    </p>
                  </div>
                </div>
                <button 
                  onClick={submitGpx}
                  disabled={!gpxFile || loading}
                  className="w-full bg-blue-600 text-white py-2 rounded-lg hover:bg-blue-700 disabled:opacity-50 font-medium"
                >
                  Upload Track
                </button>
              </div>
            )}
          </div>
        </div>

        {/* Feed */}
        <div className="space-y-4">
          <h2 className="text-lg font-semibold text-gray-800">Recent Activity</h2>
          {activities.length === 0 ? (
            <div className="text-center py-10 text-gray-500">No activity logged yet.</div>
          ) : (
            activities.map(act => (
              <div key={act.id} className="bg-white p-4 rounded-xl shadow-sm border flex items-start gap-4">
                <div className={`p-3 rounded-full shrink-0 ${
                  act.type === 'weight' ? 'bg-purple-100 text-purple-600' :
                  act.type === 'food' ? 'bg-green-100 text-green-600' :
                  act.type === 'workout' ? 'bg-orange-100 text-orange-600' :
                  'bg-blue-100 text-blue-600'
                }`}>
                  {act.type === 'weight' && <Scale size={20} />}
                  {act.type === 'food' && <Utensils size={20} />}
                  {act.type === 'workout' && <Dumbbell size={20} />}
                  {act.type === 'gpx' && <Map size={20} />}
                </div>
                <div className="flex-grow min-w-0">
                  <div className="flex justify-between items-start">
                    <h3 className="font-medium text-gray-900 capitalize">{act.type}</h3>
                    <span className="text-xs text-gray-500">{new Date(act.timestamp).toLocaleString()}</span>
                  </div>
                  <div className="mt-1 text-sm text-gray-600">
                    {act.type === 'weight' && <p>{act.data.weight} {act.data.unit}</p>}
                    {act.type === 'food' && <p>{act.data.name} - {act.data.calories} kcal</p>}
                    {act.type === 'workout' && (
                      <div>
                        <p>{act.data.type} ({act.data.duration})</p>
                        {act.data.metadata && Object.keys(act.data.metadata).length > 0 && (
                          <div className="mt-1 flex flex-wrap gap-1">
                            {Object.entries(act.data.metadata).map(([k, v]) => (
                              <span key={k} className="inline-block px-2 py-0.5 bg-gray-100 rounded text-xs text-gray-500">
                                {k}: {v as string}
                              </span>
                            ))}
                          </div>
                        )}
                      </div>
                    )}
                    {act.type === 'gpx' && <p>Track: {act.data.filename}</p>}
                  </div>
                </div>
              </div>
            ))
          )}
        </div>
      </main>
    </div>
  );
}
