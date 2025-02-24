import {
  Image,
  StyleSheet,
  Platform,
  Pressable,
  View,
  ScrollView,
  TouchableOpacity,
} from "react-native";

import { HelloWave } from "@/components/HelloWave";
import ParallaxScrollView from "@/components/ParallaxScrollView";
import { ThemedText } from "@/components/ThemedText";
import { ThemedView } from "@/components/ThemedView";
import { useAtomValue } from "jotai";
import { useQuery } from "@tanstack/react-query";
import { Button, Card, List, Title, TouchableRipple } from "react-native-paper";
import { RecentNodesResponse } from "@@/backend/bindings/RecentNodesResponse";
import { Link, router, useSegments } from "expo-router";
import { homeserverUrlAtom, useFetchApi } from "@/lib/node";

export default function HomeScreen() {
  const homeserverUrl = useAtomValue(homeserverUrlAtom);
  const fetchApi = useFetchApi();

  const segments = useSegments();

  const { data: recentNodes } = useQuery({
    queryKey: ["recentNodes"],
    queryFn: async () => fetchApi<RecentNodesResponse>(`/node/recent`),
    staleTime: 1000,
  });

  return (
    <ScrollView style={styles.container}>
      <Card>
        <Card.Content>
          <Title>
            Hello World! <HelloWave />
          </Title>
        </Card.Content>
      </Card>

      {recentNodes === undefined ? (
        <ThemedText>Loading...</ThemedText>
      ) : (
        <List.Section>
          <List.Subheader>Recent Nodes</List.Subheader>
          {recentNodes.nodes.map((node, idx) => (
            <Link push href={`/node/${node.id}`} key={node.id} asChild>
              <TouchableRipple>
                <List.Item
                  title={node.id}
                  left={(props) => <List.Icon {...props} icon="dots-circle" />}
                />
              </TouchableRipple>
            </Link>
          ))}
        </List.Section>
      )}
      <ThemedText>Your server url is {homeserverUrl}</ThemedText>
    </ScrollView>
  );
}

const styles = StyleSheet.create({
  container: { padding: 16 },
});
