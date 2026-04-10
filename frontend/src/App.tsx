import React, { useState, useEffect } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { 
  LineChart, Line, XAxis, YAxis, CartesianGrid, Tooltip, ResponsiveContainer 
} from 'recharts';
import { Layout, Weight, BarChart2, Plus, Settings, X } from 'lucide-react';

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
            className="border p-1 rounded flex-1 text-sm"
            placeholder="75.5"
          />
          <button
            onClick={() => mutation.mutate(parseFloat(weight))}
            className="bg-blue-600 text-white px-3 py-1 rounded text-sm hover:bg-blue-700"
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
          <h3 className="font-semibold flex items-center gap-2 text-sm"><BarChart2 size={18}/> {title}</h3>
          <select 
            value={activeTimeRange}
            onChange={(e) => updateConfig.mutate(e.target.value)}
            className="text-[10px] border rounded p-0.5 bg-gray-50 outline-none"
          >
            {timeOptions.map(opt => <option key={opt} value={opt}>{opt}</option>)}
          </select>
        </div>
        <div className="flex-1 min-h-0">
          {isLoading ? <p className="text-xs">Loading...</p> : (
            <ResponsiveContainer width="100%" height="100%">
              <LineChart data={chartData}>
                <CartesianGrid strokeDasharray="3 3" />
                <XAxis 
                  dataKey="timestamp" 
                  tickFormatter={(t) => new Date(t).toLocaleDateString()}
                  fontSize={9}
                />
                <YAxis fontSize={9} />
                <Tooltip 
                  labelFormatter={(t) => new Date(t).toLocaleString()}
                  contentStyle={{ fontSize: '10px' }}
                />
                <Line type="monotone" dataKey="value" stroke="#2563eb" strokeWidth={2} dot={false} />
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
  const [isPickerOpen, setIsPickerOpen] = useState(false);
  const queryClient = useQueryClient();

  const { data: config, isLoading: isConfigLoading } = useQuery({
    queryKey: ['config'],
    queryFn: async () => {
      const res = await fetch(`${API_BASE}/config`);
      return res.json();
    },
  });

  const { data: apps } = useQuery({
    queryKey: ['apps'],
    queryFn: async () => {
      const res = await fetch(`${API_BASE}/apps`);
      return res.json();
    },
  });

  const addWidget = useMutation({
    mutationFn: async ({ tabId, widget }: any) => {
      const res = await fetch(`${API_BASE}/config/widget/${tabId}`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(widget),
      });
      return res.json();
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['config'] });
      setIsPickerOpen(false);
    },
  });

  useEffect(() => {
    if (config?.tabs?.length > 0 && !activeTabId) {
      setActiveTabId(config.tabs[0].id);
    }
  }, [config]);

  if (isConfigLoading) return <div className="p-8">Loading Config...</div>;

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
      <div className="flex-1 flex flex-col overflow-hidden relative">
        <header className="bg-white border-b p-4 flex justify-between items-center shadow-sm z-10">
          <h2 className="text-xl font-semibold">{activeTab?.title || 'Dashboard'}</h2>
          <button 
            onClick={() => setIsPickerOpen(true)}
            className="bg-slate-800 text-white p-2 rounded-full hover:bg-slate-700 transition"
            title="Add Widget"
          >
            <Plus size={20} />
          </button>
        </header>

        <main className="flex-1 overflow-hidden p-6">
          {!activeTab ? (
            <div className="flex flex-col items-center justify-center h-full text-gray-400">
              <Layout size={48} className="mb-2 opacity-20" />
              <p>No tab selected or config not found.</p>
            </div>
          ) : (
            <div 
              className="grid grid-cols-12 gap-6 h-full"
              style={{ 
                gridAutoRows: 'minmax(0, 1fr)',
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
                      gridColumn: `span ${grid?.w || 3}`,
                      gridRow: `span ${grid?.h || 1}`,
                    }}
                    className="min-h-0"
                  >
                    <WidgetComp {...widget} tabId={activeTab.id} />
                  </div>
                );
              })}
            </div>
          )}
        </main>

        {/* Widget Picker Modal */}
        {isPickerOpen && (
          <div className="absolute inset-0 bg-black/50 flex items-center justify-center z-50 p-4">
            <div className="bg-white rounded-xl shadow-2xl w-full max-w-md overflow-hidden">
              <div className="p-4 border-b flex justify-between items-center bg-gray-50">
                <h3 className="font-bold">Add Widget to {activeTab?.title}</h3>
                <button onClick={() => setIsPickerOpen(false)} className="text-gray-400 hover:text-gray-600">
                  <X size={20} />
                </button>
              </div>
              <div className="p-4 flex flex-col gap-3">
                <button 
                  onClick={() => activeTab && addWidget.mutate({
                    tabId: activeTab.id,
                    widget: {
                      type: 'built-in-graph',
                      title: 'New Weight Graph',
                      query: 'weight_kg',
                      timeRange: '7d',
                      grid: { w: 9, h: 1 }
                    }
                  })}
                  className="flex items-center gap-3 p-3 border rounded-lg hover:bg-blue-50 hover:border-blue-200 transition text-left"
                >
                  <div className="bg-blue-100 p-2 rounded-lg text-blue-600"><BarChart2 size={20}/></div>
                  <div>
                    <div className="font-semibold">Weight Graph</div>
                    <div className="text-xs text-gray-500">Visualize weight metric from database</div>
                  </div>
                </button>
                <button 
                  onClick={() => activeTab && addWidget.mutate({
                    tabId: activeTab.id,
                    widget: {
                      type: 'weight-tracker-input',
                      title: 'Weight Logger',
                      grid: { w: 3, h: 1 }
                    }
                  })}
                  className="flex items-center gap-3 p-3 border rounded-lg hover:bg-green-50 hover:border-green-200 transition text-left"
                >
                  <div className="bg-green-100 p-2 rounded-lg text-green-600"><Weight size={20}/></div>
                  <div>
                    <div className="font-semibold">Weight Logger</div>
                    <div className="text-xs text-gray-500">Form to log new weight entries</div>
                  </div>
                </button>
              </div>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}

export default App;
