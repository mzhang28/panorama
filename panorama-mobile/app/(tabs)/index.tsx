import { Image, StyleSheet, Platform, Pressable } from "react-native";

import { HelloWave } from "@/components/HelloWave";
import ParallaxScrollView from "@/components/ParallaxScrollView";
import { ThemedText } from "@/components/ThemedText";
import { ThemedView } from "@/components/ThemedView";
import { useAtomValue } from "jotai";
import { homeserverUrlAtom } from "./settings";
import { useQuery } from "@tanstack/react-query";
import { Button, Card, List, Title } from "react-native-paper";
import { RecentNodesResponse } from "@@/backend/bindings/RecentNodesResponse";
import { Link } from "expo-router";

async function fetchApi<T>(base: string): Promise<T> {
  const res = await fetch(`${base}`);
  const data: T = await res.json();
  return data;
}

export default function HomeScreen() {
  const homeserverUrl = useAtomValue(homeserverUrlAtom);

  const { data: recentNodes } = useQuery({
    queryKey: ["recentNodes", homeserverUrl],
    queryFn: async () =>
      fetchApi<RecentNodesResponse>(`${homeserverUrl}/node/recent`),
    staleTime: 1000,
  });

  return (
    <ParallaxScrollView
      headerBackgroundColor={{ light: "#A1CEDC", dark: "#1D3D47" }}
      headerImage={
        <Image
          source={require("@/assets/images/partial-react-logo.png")}
          style={styles.reactLogo}
        />
      }
    >
      <Card>
        <Card.Content>
          <Title>Hello World!</Title>
        </Card.Content>
      </Card>

      {recentNodes === undefined ? (
        <ThemedText>Loading...</ThemedText>
      ) : (
        <List.Section>
          <List.Subheader>Recent Nodes</List.Subheader>
          {recentNodes.nodes.map((node, idx) => (
            <Link push href="/(tabs)/settings" key={node.id}>
              {node.id}
              {/* <List.Item title={node.id} /> */}
            </Link>
          ))}
        </List.Section>
      )}

      <ThemedView style={styles.titleContainer}>
        <ThemedText type="title">hellosu.</ThemedText>
        <HelloWave />
      </ThemedView>
      <ThemedText>Your server url is {homeserverUrl}</ThemedText>
      <ThemedView style={styles.stepContainer}>
        <ThemedText type="subtitle">Step 1: Try it</ThemedText>
        <ThemedText>
          Edit{" "}
          <ThemedText type="defaultSemiBold">app/(tabs)/index.tsx</ThemedText>{" "}
          to see changes. Press{" "}
          <ThemedText type="defaultSemiBold">
            {Platform.select({
              ios: "cmd + d",
              android: "cmd + m",
              web: "F12",
            })}
          </ThemedText>{" "}
          to open developer tools.
        </ThemedText>
      </ThemedView>
      <ThemedView style={styles.stepContainer}>
        <ThemedText type="subtitle">Step 2: Explore</ThemedText>
        <ThemedText>
          Tap the Explore tab to learn more about what's included in this
          starter app.
        </ThemedText>
      </ThemedView>
      <ThemedView style={styles.stepContainer}>
        <ThemedText type="subtitle">Step 3: Get a fresh start</ThemedText>
        <ThemedText>
          When you're ready, run{" "}
          <ThemedText type="defaultSemiBold">npm run reset-project</ThemedText>{" "}
          to get a fresh <ThemedText type="defaultSemiBold">app</ThemedText>{" "}
          directory. This will move the current{" "}
          <ThemedText type="defaultSemiBold">app</ThemedText> to{" "}
          <ThemedText type="defaultSemiBold">app-example</ThemedText>.
        </ThemedText>
      </ThemedView>
    </ParallaxScrollView>
  );
}

const styles = StyleSheet.create({
  titleContainer: {
    flexDirection: "row",
    alignItems: "center",
    gap: 8,
  },
  stepContainer: {
    gap: 8,
    marginBottom: 8,
  },
  reactLogo: {
    height: 178,
    width: 290,
    bottom: 0,
    left: 0,
    position: "absolute",
  },
});
