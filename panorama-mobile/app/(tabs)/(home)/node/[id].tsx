import { ThemedText } from "@/components/ThemedText";
import { useFetchApi } from "@/lib/node";
import { useQuery } from "@tanstack/react-query";
import { useLocalSearchParams } from "expo-router";
import { View, StyleSheet } from "react-native";
import { Text } from "react-native-paper";

export interface NodePageProps {}

export default function NodePage({}: NodePageProps) {
  const { id } = useLocalSearchParams();
  const fetchApi = useFetchApi();

  const { data: nodeInfo } = useQuery({
    queryKey: ["node", id],
    queryFn: async () => fetchApi(`/node/${id}`),
    staleTime: 1000,
  });

  return (
    <View>
      <ThemedText>Node {id}</ThemedText>

      <Text style={styles.code}>{JSON.stringify(nodeInfo, null, 2)}</Text>
    </View>
  );
}

const styles = StyleSheet.create({
  code: {
    fontFamily: "monospace",
    fontSize: 14,
    color: "#666",
  },
});
