export interface IndexedField {
  name: string;
  description: string;
  type: 'string' | 'number' | 'boolean';
  indexed: boolean;
}

export interface AppManifest {
  name: string;
  description: string;
  version: string;
  indexedFields: IndexedField[];
  nodeType: string;
}