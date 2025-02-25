import { Tabs } from "expo-router";
import React from "react";
import { Platform } from "react-native";

import { HapticTab } from "@/components/HapticTab";
import { IconSymbol } from "@/components/ui/IconSymbol";
import TabBarBackground from "@/components/ui/TabBarBackground";
import { Colors } from "@/constants/Colors";
import { useColorScheme } from "@/hooks/useColorScheme";
import { createBottomTabNavigator } from "@react-navigation/bottom-tabs";
import { BottomNavigation, Icon } from "react-native-paper";
import { CommonActions } from "@react-navigation/native";

export default function TabLayout() {
  const colorScheme = useColorScheme();
  return (
    <Tabs
      tabBar={({ navigation, state, descriptors }) => (
        <BottomNavigation.Bar
          shifting
          navigationState={state}
          onTabPress={({ route, preventDefault }) => {
            const event = navigation.emit({
              type: "tabPress",
              target: route.key,
              canPreventDefault: true,
            });

            if (event.defaultPrevented) {
              preventDefault();
            } else {
              navigation.dispatch({
                ...CommonActions.navigate(route.name, route.params),
                target: state.key,
              });
            }
          }}
          renderIcon={({ route, focused, color }) => {
            const { options } = descriptors[route.key];
            if (options.tabBarIcon) {
              return options.tabBarIcon({ focused, color, size: 24 });
            }

            return null;
          }}
          getLabelText={({ route }) => {
            const { options } = descriptors[route.key];
            const label =
              options.tabBarLabel !== undefined
                ? options.tabBarLabel
                : options.title !== undefined
                  ? options.title
                  : route.title;

            return label;
          }}
        />
      )}
    >
      <Tabs.Screen
        name="index"
        options={{
          title: "Home",
          tabBarIcon: ({ color }) => (
            <Icon source="home" color={color} size={24} />
          ),
        }}
      />
      <Tabs.Screen
        name="explore"
        options={{
          title: "Home",
          tabBarIcon: ({ color }) => (
            <Icon source="home" color={color} size={24} />
          ),
        }}
      />
      <Tabs.Screen
        name="settings"
        options={{
          title: "Home",
          tabBarIcon: ({ color }) => (
            <Icon source="home" color={color} size={24} />
          ),
        }}
      />
    </Tabs>
  );
}

//   return (
//     <Tab.Navigator
//       tabBar={({ navigation, state, descriptors }) => (
//         <BottomNavigation.Bar
//           navigationState={state}
//           onTabPress={({ route, preventDefault }) => {
//             const event = navigation.emit({
//               type: "tabPress",
//               target: route.key,
//               canPreventDefault: true,
//             });

//             if (event.defaultPrevented) {
//               preventDefault();
//             } else {
//               navigation.dispatch({
//                 ...CommonActions.navigate(route.name, route.params),
//                 target: state.key,
//               });
//             }
//           }}
//           renderIcon={({ route, focused, color }) => {
//             const { options } = descriptors[route.key];
//             if (options.tabBarIcon) {
//               return options.tabBarIcon({ focused, color, size: 24 });
//             }
//             return null;
//           }}
//           getLabelText={({ route }) => {
//             const { options } = descriptors[route.key];
//             const label =
//               options.tabBarLabel !== undefined
//                 ? options.tabBarLabel
//                 : options.title !== undefined
//                   ? options.title
//                   : route.title;
//             return label;
//           }}
//         />
//       )}
//     >
//       <Tab.Screen
//         name="Home"
//         component={HomeScreen}
//         options={{
//           tabBarLabel: "Home",
//           tabBarIcon: ({ color, size }) => (
//             <Icon source="home" size={size} color={color} />
//           ),
//         }}
//       />
//       <Tab.Screen
//         name="Settings"
//         component={SettingsScreen}
//         options={{
//           tabBarLabel: "Settings",
//           tabBarIcon: ({ color, size }) => (
//             <Icon source="cog" size={size} color={color} />
//           ),
//         }}
//       />
//     </Tab.Navigator>
//     // <Tabs
//     //   screenOptions={{
//     //     tabBarActiveTintColor: Colors[colorScheme ?? "light"].tint,
//     //     headerShown: false,
//     //     tabBarButton: HapticTab,
//     //     tabBarBackground: TabBarBackground,
//     //     tabBarStyle: Platform.select({
//     //       ios: {
//     //         // Use a transparent background on iOS to show the blur effect
//     //         position: "absolute",
//     //       },
//     //       default: {},
//     //     }),
//     //   }}
//     // >
//     //   <Tabs.Screen
//     //     name="index"
//     //     options={{
//     //       title: "Home",
//     //       tabBarIcon: ({ color }) => (
//     //         <IconSymbol size={28} name="house.fill" color={color} />
//     //       ),
//     //     }}
//     //   />
//     //   <Tabs.Screen
//     //     name="explore"
//     //     options={{
//     //       title: "Explore",
//     //       tabBarIcon: ({ color }) => (
//     //         <IconSymbol size={28} name="paperplane.fill" color={color} />
//     //       ),
//     //     }}
//     //   />
//     //   <Tabs.Screen
//     //     name="settings"
//     //     options={{
//     //       title: "Settings",
//     //       tabBarIcon: ({ color }) => (
//     //         <IconSymbol size={28} name="settings.fill" color={color} />
//     //       ),
//     //     }}
//     //   />
//     // </Tabs>
//   );
// }
