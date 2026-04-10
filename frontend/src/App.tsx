import React, { useState, useEffect } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { 
  LineChart, Line, XAxis, YAxis, CartesianGrid, Tooltip, ResponsiveContainer 
} from 'recharts';
import { Layout, Weight, BarChart2, Plus, Settings } from 'lucide-react';

const API_BASE = 'http://localhost:3001/api';

// Widget Registry (logical third-party apps hardcoded for now)
const WidgetRegistry: Record<string, React.FC<any>> = {
  'weight-tracker-input': ({ id }) => {
    const [weight, setWeight] = useState('');
    const queryClient = useQueryClient();

    const mutation = useMutation({
      mutationFn: async (val: number) => {
        const res = await fetch(`${API_BASE}/apps/weight-tracker/entry`, {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ weight: val }),
        });
        return res.json();
      },
      onSuccess: () => {
        queryClient.invalidateQueries({ queryKey: ['query'] });
        setWeight('');
      },
    });

    return (
      <div className="flex flex-col gap-2 p-4 border rounded bg-white shadow-sm h-full">
        <h3 className="font-semibold flex items-center gap-2"><Weight size={18}/> Log Weight (kg)</h3>
        <div className="flex gap-2">
          <input
            type="number"
            value={weight}
            onChange={(e) => setWeight(e.target.value)}
            className="border p-1 rounded flex-1"
            placeholder="75.5"
          />
          <button
            onClick={() => mutation.mutate(parseFloat(weight))}
            className="bg-blue-600 text-white px-3 py-1 rounded hover:bg-blue-700"
            disabled={mutation.isPending}
          >
            Add
          </button>
        </div>
      </div>
    );
  },

  'built-in-graph': ({ id, title, query: queryString, timeRange, tabId }) => {
    const queryClient = useQueryClient();
    const activeTimeRange = timeRange || '7d';

    const { data: chartData, isLoading } = useQuery({
      queryKey: ['query', queryString, activeTimeRange],
      queryFn: async () => {
        const res = await fetch(`${API_BASE}/query?q=${queryString}&t=${activeTimeRange}`);
        return res.json();
      },
    });

    const updateConfig = useMutation({
      mutationFn: async (newRange: string) => {
        await fetch(`${API_BASE}/config/widget/${tabId}/${id}`, {
          method: 'PUT',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ timeRange: newRange }),
        });
      },
      onSuccess: () => {
        queryClient.invalidateQueries({ queryKey: ['config'] });
      },
    });

    const timeOptions = ['1h', '6h', '24h', '3d', '7d', '30d', 'all'];

    return (
      <div className="flex flex-col gap-2 p-4 border rounded bg-white shadow-sm h-full overflow-hidden">
        <div className="flex justify-between items-center">
          <h3 className="font-semibold flex items-center gap-2"><BarChart2 size={18}/> {title}</h3>
          <select 
            value={activeTimeRange}
            onChange={(e) => updateConfig.mutate(e.target.value)}
            className="text-xs border rounded p-1 bg-gray-50 outline-none"
          >
            {timeOptions.map(opt => <option key={opt} value={opt}>{opt}</option>)}
          </select>
        </div>
        <div className="flex-1 min-h-0">
          {isLoading ? <p>Loading...</p> : (
            <ResponsiveContainer width="100%" height="100%">
              <LineChart data={chartData}>
                <CartesianGrid strokeDasharray="3 3" />
                <XAxis 
                  dataKey="timestamp" 
                  tickFormatter={(t) => new Date(t).toLocaleDateString()}
                  fontSize={10}
                />
                <YAxis fontSize={10} />
                <Tooltip 
                  labelFormatter={(t) => new Date(t).toLocaleString()}
                />
                <Line type="monotone" dataKey="value" stroke="#2563eb" strokeWidth={2} />
              </LineChart>
            </ResponsiveContainer>
          )}
        </div>
      </div>
    );
  }
};

function App() {
  const [activeTabId, setActiveTabId] = useState<string | null>(null);

  const { data: config, isLoading } = useQuery({
    queryKey: ['config'],
    queryFn: async () => {
      const res = await fetch(`${API_BASE}/config`);
      return res.json();
    },
  });

  useEffect(() => {
    if (config?.tabs?.length > 0 && !activeTabId) {
      setActiveTabId(config.tabs[0].id);
    }
  }, [config]);

  if (isLoading) return <div className="p-8">Loading Config...</div>;

  const activeTab = config?.tabs?.find((t: any) => t.id === activeTabId);

  return (
    <div className="flex h-screen bg-gray-50 text-gray-900 font-sans overflow-hidden">
      {/* Sidebar Tabs */}
      <div className="w-64 bg-slate-900 text-white flex flex-col p-4 gap-2">
        <div className="text-xl font-bold mb-6 flex items-center gap-2 px-2">
          <Layout className="text-blue-400" /> Panorama
        </div>
        {config?.tabs?.map((tab: any) => (
          <button
            key={tab.id}
            onClick={() => setActiveTabId(tab.id)}
            className={`text-left p-3 rounded-lg transition ${
              activeTabId === tab.id ? 'bg-blue-600 shadow-lg' : 'hover:bg-slate-800'
            }`}
          >
            {tab.title}
          </button>
        ))}
        <div className="mt-auto pt-4 border-t border-slate-800">
           <button className="flex items-center gap-2 p-2 text-slate-400 hover:text-white transition">
             <Settings size={18} /> Settings
           </button>
        </div>
      </div>

      {/* Dashboard Area */}
      <div className="flex-1 flex flex-col overflow-hidden">
        <header className="bg-white border-b p-4 flex justify-between items-center shadow-sm z-10">
          <h2 className="text-xl font-semibold">{activeTab?.title || 'Dashboard'}</h2>
          <button className="bg-slate-800 text-white p-2 rounded-full hover:bg-slate-700 transition">
            <Plus size={20} />
          </button>
        </header>

        <main className="flex-1 overflow-auto p-6">
          <div 
            className="grid grid-cols-12 gap-6"
            style={{ 
              gridAutoRows: 'minmax(100px, auto)',
            }}
          >
            {activeTab?.widgets?.map((widget: any) => {
              const WidgetComp = WidgetRegistry[widget.type];
              if (!WidgetComp) return <div key={widget.id}>Unknown Widget: {widget.type}</div>;

              const { grid } = widget;
              return (
                <div 
                  key={widget.id}
                  style={{
                    gridColumn: `span ${grid?.w || 4}`,
                    gridRow: `span ${grid?.h || 2}`,
                  }}
                  className="min-h-[200px]"
                >
                  <WidgetComp {...widget} tabId={activeTab.id} />
                </div>
              );
            })}
          </div>
        </main>
      </div>
    </div>
  );
}

export default App;
