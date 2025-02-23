import { fetchNodeTags } from "@/lib/node";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useState, type FormEvent } from "react";
import { useCallback } from "react";

export interface TagEditorProps {
  id: string;
}

export default function TagEditor({ id }: TagEditorProps) {
  const queryClient = useQueryClient();
  const { data: tags } = useQuery({
    queryKey: ["node", id, "tags"],
    queryFn: () => fetchNodeTags(id),
  });

  const [newTagValue, setNewTagValue] = useState("");

  const { mutate: addTagMutate } = useMutation({
    mutationKey: ["node", id, "tags"],
    mutationFn: async (newTagValue: string) => {
      const res = await fetch(`/api/node/${id}/tags`, {
        method: "PATCH",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ insertions: [newTagValue] }),
      });
      queryClient.invalidateQueries({ queryKey: ["node", id] });
    },
  });

  const addTag = useCallback(
    (e: FormEvent) => {
      e.preventDefault();
      addTagMutate(newTagValue);
      setNewTagValue("");
    },
    [newTagValue],
  );

  return (
    <div className="flex">
      <form onSubmit={addTag}>
        <input
          placeholder="Enter tags"
          className="outline-0 p-1 text-right"
          value={newTagValue}
          onChange={(e) => setNewTagValue(e.target.value)}
        />
      </form>
      <div className="flex p-1 gap-1">
        {(tags ?? []).map((tag) => (
          <div className="bg-slate-300 px-1">{tag}</div>
        ))}
      </div>
    </div>
  );
}
