export interface Block {
  id: string;
  fields: Record<string, { type: string; value: any }>;
  created_at: string;
  updated_at: string;
}

export interface BlockNode {
  n?: Block;
  id?: string;
  fields?: Record<string, { type: string; value: any }>;
  created_at?: string;
  updated_at?: string;
}

const PLUGIN_ID = "io.mzhang.panorama.journal";

export async function callPluginEndpoint(
  pluginId: string,
  endpoint: string,
  method = "GET",
  body?: unknown,
): Promise<Response> {
  const opts: RequestInit = {
    method,
    headers: body ? { "Content-Type": "application/json" } : {},
    body: body ? JSON.stringify(body) : undefined,
  };
  return fetch(`/plugin/${pluginId}/${endpoint}`, opts);
}

export function f(node: BlockNode, key: string): any {
  return (node?.n || node)?.fields?.[key]?.value;
}

export function fStr(node: BlockNode, key: string): string {
  const v = f(node, key);
  return v != null ? String(v) : "";
}

export function fBool(node: BlockNode, key: string): boolean {
  const v = f(node, key);
  return v === true || v === "true";
}

export function fJson(node: BlockNode, key: string): any {
  const v = f(node, key);
  if (typeof v === "string") {
    try {
      return JSON.parse(v);
    } catch {
      return v;
    }
  }
  return v;
}

export function normalizeBlock(raw: any): Block {
  return raw?.n || raw;
}

export async function api(
  path: string,
  method = "GET",
  body?: any,
): Promise<any> {
  const res = await callPluginEndpoint(PLUGIN_ID, path, method, body);
  if (!res.ok) throw new Error(await res.text());
  return res.json();
}
