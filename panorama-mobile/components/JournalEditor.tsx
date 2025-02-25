import { useFetchApi } from "@/lib/node";
import {
  MarkdownTextInput,
  parseExpensiMark,
} from "@expensify/react-native-live-markdown";
import { useQuery } from "@tanstack/react-query";
import { useCallback, useEffect, useState } from "react";
import { StyleSheet } from "react-native";
import { Text } from "react-native-paper";

export interface JournalEditorProps {
  id: string;
  date: string;
}

export default function JournalEditor({ id, date }: JournalEditorProps) {
  const [lastServerPage, setLastServerPage] = useState<string | null>(null);
  const [localPage, setLocalPage] = useState("");
  const fetchApi = useFetchApi();

  const { data: nodeJson, status } = useQuery({
    queryKey: ["node", id, "fetch"],
    queryFn: () => fetchApi(`/node/${id}`),
  });

  const savePage = useCallback(
    async (value: string) => {
      console.log("DATE IS", date);
      await fetchApi(`/apps/journal/by_date/${date}`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ content: value }),
      });
    },
    [date],
  );

  useEffect(() => {
    if (!nodeJson) return;
    const { content } = nodeJson;

    // This is the first load from the server, so let's load it in
    if (lastServerPage === null && content !== null && content !== undefined) {
      setLastServerPage(content);
      setLocalPage(content);
      // mdxEditorRef.current?.setMarkdown(content);
    }

    // DON'T DO ANYTHING IF THE LOCAL PAGE HAS BEEN TOUCHED!
    // TODO: Implement CRDT
    if (lastServerPage === localPage) {
      setLastServerPage(content);
      setLocalPage(content);
      // mdxEditorRef.current?.setMarkdown(content);
    }
  }, [nodeJson, lastServerPage, localPage]);

  const updateLocalPage = useCallback((value: string) => {
    setLocalPage(value);
    // mdxEditorRef.current?.setMarkdown(value);
    savePage(value);
  }, []);

  if (!nodeJson) return <Text>Loading... ({status})</Text>;

  console.log("localPage", localPage);

  return (
    <MarkdownTextInput
      style={styles.contentEditor}
      value={localPage}
      onChange={(e) => updateLocalPage(e.nativeEvent.text)}
      parser={parseExpensiMark}
      multiline
    />
  );
}

const styles = StyleSheet.create({
  contentEditor: {
    height: 200,
    borderWidth: 1,
    backgroundColor: "#fff",
    borderColor: "#ccc",
    borderRadius: 4,
    padding: 8,
    verticalAlign: "top",
  },
});
