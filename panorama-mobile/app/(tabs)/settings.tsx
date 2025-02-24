import { ThemedText } from "@/components/ThemedText";
import {
  SafeAreaView,
  ScrollView,
  StyleSheet,
  TextInput,
  View,
} from "react-native";
import { atom, useAtom } from "jotai";

export const homeserverUrlAtom = atom("");

export default function SettingsScreen() {
  const [homeserverUrl, setHomeserverUrl] = useAtom(homeserverUrlAtom);

  return (
    // <SafeAreaView>
    //   <ScrollView>
    <View style={styles.container}>
      <ThemedText style={styles.titleText}>HELLOSUS</ThemedText>
      <TextInput
        keyboardType="url"
        autoCapitalize="none"
        placeholder="Enter a homeserver URL..."
        style={styles.homeserverInput}
        value={homeserverUrl}
        onChange={(e) => setHomeserverUrl(e.nativeEvent.text)}
      />
    </View>
    //   </ScrollView>
    // </SafeAreaView>
  );
}

const styles = StyleSheet.create({
  container: {
    paddingTop: 100,
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
