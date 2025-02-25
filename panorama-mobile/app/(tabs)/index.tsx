import {
  Image,
  StyleSheet,
  Platform,
  Pressable,
  Text,
  View,
  ScrollView,
  TouchableOpacity,
  SafeAreaView,
} from "react-native";

import { HelloWave } from "@/components/HelloWave";
import ParallaxScrollView from "@/components/ParallaxScrollView";
import { ThemedText } from "@/components/ThemedText";
import { ThemedView } from "@/components/ThemedView";
import { useAtomValue } from "jotai";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import {
  Button,
  Card,
  FAB,
  List,
  Title,
  TouchableRipple,
} from "react-native-paper";
import { RecentNodesResponse } from "@@/backend/bindings/RecentNodesResponse";
import { Link, router, useRootNavigationState, useSegments } from "expo-router";
import { homeserverUrlAtom, useApiQuery, useFetchApi } from "@/lib/node";
import NodeListItem from "@/components/NodeListItem";

export default function HomeScreen() {
  const homeserverUrl = useAtomValue(homeserverUrlAtom);
  const fetchApi = useFetchApi();
  const queryClient = useQueryClient();

  const results = useApiQuery({
    queryKey: ["recentNodes"],
    queryFn: async () => fetchApi<RecentNodesResponse>(`/node/recent`),
    staleTime: 1000,
  });

  const { data: recentNodes, status } = results;

  recentNodes?.nodes?.sort((a, b) =>
    b.last_updated_at?.localeCompare(a.last_updated_at),
  );

  return (
    <SafeAreaView style={styles.safeAreaView}>
      <ScrollView style={styles.container}>
        <Button
          mode="contained"
          onPress={() => queryClient.invalidateQueries({ queryKey: [] })}
        >
          Refresh
        </Button>

        {recentNodes === undefined ? (
          <>
            <ThemedText>Loading ({status})...</ThemedText>
            <Text>{JSON.stringify(results, null, 2)}</Text>
          </>
        ) : (
          <List.Section>
            <List.Subheader>Recent Nodes</List.Subheader>
            {recentNodes.nodes.map((node, idx) => (
              <Link push href={`/node/view/${node.id}`} key={node.id} asChild>
                <TouchableRipple>
                  <NodeListItem node={node} />
                </TouchableRipple>
              </Link>
            ))}
          </List.Section>
        )}

        <ThemedText>Your server url is {homeserverUrl}</ThemedText>
      </ScrollView>
      <Link push href="/node/create" asChild>
        <FAB style={styles.fab} icon="plus" />
      </Link>
    </SafeAreaView>
  );
}

const styles = StyleSheet.create({
  safeAreaView: { flex: 1 },
  container: { padding: 16 },
  fab: {
    position: "absolute",
    margin: 16,
    right: 0,
    bottom: 0,
  },
});
