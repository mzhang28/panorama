import { ThemedText } from "@/components/ThemedText";
import {
  SafeAreaView,
  ScrollView,
  StyleSheet,
  TextInput,
  View,
} from "react-native";
import { atom, useAtom } from "jotai";
import { homeserverUrlAtom } from "@/lib/node";
import { Text, Title } from "react-native-paper";

export default function SettingsScreen() {
  const [homeserverUrl, setHomeserverUrl] = useAtom(homeserverUrlAtom);

  return (
    <View style={styles.container}>
      <Title>HELLOSUS</Title>

      <Text>Homeserver URL</Text>
      <TextInput
        keyboardType="url"
        autoCapitalize="none"
        placeholder="Enter a homeserver URL..."
        style={styles.homeserverInput}
        value={homeserverUrl}
        onChange={(e) => setHomeserverUrl(e.nativeEvent.text)}
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
