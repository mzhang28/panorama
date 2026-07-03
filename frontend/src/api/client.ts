const BASE = '';

export interface Node {
  id: string;
  fields: Record<string, FieldValue>;
  space_id: string;
  preferred_schemas: SchemaRef[];
  created_at: string;
  updated_at: string;
}

export interface FieldValue {
  type: string;
  value: any;
}

export interface SchemaRef {
  schema_node_id: string;
  version: { major: number; minor: number };
}

export interface PluginInfo {
  id: string;
  name: string;
  version: string;
  description: string;
  endpoints: HttpEndpointDef[];
  ui_components: UiComponentDef[];
}

export interface HttpEndpointDef {
  method: string;
  path: string;
  description: string;
}

export interface UiComponentDef {
  id: string;
  name: string;
  mount_point: { type: string; value?: string } | string;
  bundle_path: string;
}

// Node API
export async function createNode(fields: Record<string, FieldValue>, spaceId?: string): Promise<Node> {
  const res = await fetch(`${BASE}/api/nodes`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ fields, space_id: spaceId }),
  });
  if (!res.ok) throw new Error(await res.text());
  return res.json();
}

export async function getNode(id: string): Promise<Node> {
  const res = await fetch(`${BASE}/api/nodes/${id}`);
  if (!res.ok) throw new Error(await res.text());
  return res.json();
}

export async function updateNode(id: string, fields: Record<string, FieldValue>): Promise<Node> {
  const res = await fetch(`${BASE}/api/nodes/${id}`, {
    method: 'PUT',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ fields }),
  });
  if (!res.ok) throw new Error(await res.text());
  return res.json();
}

export async function deleteNode(id: string): Promise<void> {
  const res = await fetch(`${BASE}/api/nodes/${id}`, { method: 'DELETE' });
  if (!res.ok) throw new Error(await res.text());
}

export async function queryNodes(params: Record<string, string> = {}): Promise<Node[]> {
  const query = new URLSearchParams(params).toString();
  const res = await fetch(`${BASE}/api/nodes?${query}`);
  if (!res.ok) throw new Error(await res.text());
  return res.json();
}

// Schema API
export async function listSchemas(): Promise<any[]> {
  const res = await fetch(`${BASE}/api/schemas`);
  return res.json();
}

// Plugin API
export async function listPlugins(): Promise<PluginInfo[]> {
  const res = await fetch(`${BASE}/api/plugins`);
  return res.json();
}

export async function getPlugin(id: string): Promise<PluginInfo> {
  const res = await fetch(`${BASE}/api/plugins/${id}`);
  if (!res.ok) throw new Error(await res.text());
  return res.json();
}

// Object storage API
export async function uploadObject(bucket: string, key: string, data: Blob): Promise<any> {
  const res = await fetch(`${BASE}/api/objects/${bucket}/${key}`, {
    method: 'PUT',
    body: data,
  });
  return res.json();
}

// Generic plugin endpoint call
export async function callPluginEndpoint(
  pluginId: string,
  endpoint: string,
  method: string = 'GET',
  body?: any,
): Promise<Response> {
  const url = `${BASE}/plugin/${pluginId}/${endpoint}`;
  const opts: RequestInit = {
    method,
    headers: body ? { 'Content-Type': 'application/json' } : {},
    body: body ? JSON.stringify(body) : undefined,
  };
  return fetch(url, opts);
}
