import React, { useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { client } from './client';
import pino from 'pino';

const logger = pino({ browser: { asObject: true } });

function App() {
  const [name, setName] = useState('');
  const queryClient = useQueryClient();

  const { data: greetings, isLoading } = useQuery({
    queryKey: ['greetings'],
    queryFn: async () => {
      const res = await fetch('http://localhost:3001/api/greetings');
      return res.json();
    },
  });

  const mutation = useMutation({
    mutationFn: async (newName: string) => {
      logger.info({ name: newName }, 'Sending RPC Greet request');
      return client.greet({ name: newName });
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['greetings'] });
      setName('');
    },
  });

  return (
    <div className="min-h-screen bg-gray-100 p-8">
      <div className="max-w-md mx-auto bg-white rounded-xl shadow-md overflow-hidden p-6">
        <h1 className="text-2xl font-bold mb-4">Panorama Fullstack</h1>
        <div className="mb-4 flex gap-2">
          <input
            type="text"
            value={name}
            onChange={(e) => setName(e.target.value)}
            className="border p-2 rounded w-full"
            placeholder="Enter name"
          />
          <button
            onClick={() => mutation.mutate(name)}
            className="bg-blue-500 text-white px-4 py-2 rounded hover:bg-blue-600"
            disabled={mutation.isPending}
          >
            Greet
          </button>
        </div>

        <div>
          <h2 className="text-xl font-semibold mb-2">Greetings List (from DB)</h2>
          {isLoading ? (
            <p>Loading...</p>
          ) : (
            <ul>
              {greetings?.map((g: any) => (
                <li key={g.id} className="border-b py-2 text-gray-700">
                  <span className="font-bold">{g.name}:</span> {g.message}
                </li>
              ))}
            </ul>
          )}
        </div>
      </div>
    </div>
  );
}

export default App;
