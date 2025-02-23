export async function fetchNode(id: string) {
  const res = await fetch(`/api/node/${id}`);
  return await res.json();
}

export async function fetchNodeTags(id: string) {
  const res = await fetch(`/api/node/${id}/tags`);
  return await res.json();
}
