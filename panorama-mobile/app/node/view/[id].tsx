import { ThemedText } from "@/components/ThemedText";
import {
  MarkdownTextInput,
  parseExpensiMark,
} from "@expensify/react-native-live-markdown";
import { homeserverUrlAtom, useApiQuery, useFetchApi } from "@/lib/node";
import { useQuery } from "@tanstack/react-query";
import { useLocalSearchParams } from "expo-router";
import { View, StyleSheet } from "react-native";
import { Card, Chip, Icon, Text, Title } from "react-native-paper";
import { format } from "date-fns";
import { useCallback, useEffect, useState } from "react";
import JournalEditor from "@/components/JournalEditor";
import { useAtom } from "jotai";

export interface NodePageProps {}

export default function NodePage({}: NodePageProps) {
  const { id } = useLocalSearchParams();
  const fetchApi = useFetchApi();

  const { data: nodeInfo } = useApiQuery({
    queryKey: ["node", id],
    queryFn: async () => fetchApi(`/node/${id}`),
    staleTime: 1000,
  });

  const { data: tagInfo } = useApiQuery({
    queryKey: ["node", id, "tags"],
    queryFn: async () => fetchApi(`/node/${id}/tags`),
    staleTime: 1000,
  });

  if (!nodeInfo) return <ThemedText>Loading...</ThemedText>;

  return (
    <View style={styles.container}>
      {nodeInfo.title && <Title style={styles.title}>{nodeInfo.title}</Title>}
      <ThemedText>Node {id}</ThemedText>

      {nodeInfo.cal_date && (
        <Card>
          <Card.Content style={styles.calendarCard}>
            <Icon source="calendar" size={24} />
            <Text>
              {format(new Date(nodeInfo.cal_date), "yyyy-MM-dd HH:mm")}
            </Text>
          </Card.Content>
        </Card>
      )}

      <Card>
        <Card.Content style={styles.calendarCard}>
          {(tagInfo ?? []).map((tag) => (
            <Chip key={tag} icon="tag" mode="outlined" onPress={() => {}}>
              {tag}
            </Chip>
          ))}
        </Card.Content>
      </Card>

      {nodeInfo.journal_date !== null && nodeInfo.content !== undefined && (
        <>
          <Card>
            <Card.Content style={styles.calendarCard}>
              <Icon source="notebook" size={24} />
              <Text>{nodeInfo.journal_date}</Text>
            </Card.Content>
          </Card>
          <Text>Content:</Text>
          <JournalEditor id={id} date={nodeInfo.journal_date} />
        </>
      )}

      <Text style={styles.code}>
        {JSON.stringify(JSON.parse(nodeInfo.json ?? "{}"), null, 2)}
      </Text>
    </View>
  );
}

const styles = StyleSheet.create({
  container: {
    padding: 16,
    display: "flex",
    flexDirection: "column",
    gap: 16,
  },
  code: {
    fontFamily: "monospace",
    fontSize: 14,
    color: "#666",
  },
  title: {
    fontSize: 24,
    fontWeight: "bold",
  },
  calendarCard: {
    display: "flex",
    flexDirection: "row",
    alignItems: "center",
    gap: 8,
  },
});
