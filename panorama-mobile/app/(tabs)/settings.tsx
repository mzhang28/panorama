import { ThemedText } from "@/components/ThemedText";
import { SafeAreaView, ScrollView, StyleSheet, View } from "react-native";
import { atom, useAtom } from "jotai";
import { homeserverUrlAtom } from "@/lib/node";
import { Text, TextInput, Title } from "react-native-paper";
import { useCallback } from "react";
import { useQueries, useQueryClient } from "@tanstack/react-query";

export default function SettingsScreen() {
  const [homeserverUrl, setHomeserverUrl] = useAtom(homeserverUrlAtom);
  const queryClient = useQueryClient();

  const updateHomeserverUrl = useCallback(
    (url: string) => {
      setHomeserverUrl(url);
      queryClient.clear();
    },
    [setHomeserverUrl],
  );

  return (
    <View style={styles.container}>
      <Title>HELLOSUS</Title>

      <TextInput
        label="Homeserver URL"
        keyboardType="url"
        autoCapitalize="none"
        placeholder="Enter a homeserver URL..."
        style={styles.homeserverInput}
        value={homeserverUrl}
        onChange={(e) => updateHomeserverUrl(e.nativeEvent.text)}
      />
    </View>
  );
}

const styles = StyleSheet.create({
  container: {
    padding: 16,
    display: "flex",
    flexDirection: "column",
    gap: 20,
  },

  titleText: {
    fontSize: 36,
  },

  homeserverInput: {
    borderColor: "white",
    backgroundColor: "#333",
    padding: 4,
    color: "white",
  },
});
