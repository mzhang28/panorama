import { ThemedText } from "@/components/ThemedText";
import { useFetchApi } from "@/lib/node";
import { useQuery } from "@tanstack/react-query";
import { useLocalSearchParams } from "expo-router";
import { View, StyleSheet } from "react-native";
import { Card, Icon, Text, Title } from "react-native-paper";
import { format } from "date-fns";

export interface NodePageProps {}

export default function NodePage({}: NodePageProps) {
  const { id } = useLocalSearchParams();
  const fetchApi = useFetchApi();

  const { data: nodeInfo } = useQuery({
    queryKey: ["node", id],
    queryFn: async () => fetchApi(`/node/${id}`),
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
