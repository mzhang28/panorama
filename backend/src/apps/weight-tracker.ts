export interface AppManifest {
  id: string;
  name: string;
  version: string;
  tables: {
    name: string;
    columns: {
      name: string;
      type: 'text' | 'integer' | 'real' | 'timestamp';
      notNull?: boolean;
      references?: string;
    }[];
  }[];
}

export const weightTrackerManifest: AppManifest = {
  id: 'weight-tracker',
  name: 'Weight Tracker',
  version: '0.1.0',
  tables: [
    {
      name: 'app_weight_entries',
      columns: [
        { name: 'node_id', type: 'text', notNull: true, references: 'nodes(id)' },
        { name: 'weight_kg', type: 'real', notNull: true },
        { name: 'timestamp', type: 'integer', notNull: true },
      ],
    },
  ],
};
