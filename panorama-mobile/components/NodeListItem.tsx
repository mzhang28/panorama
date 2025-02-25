import { homeserverUrlAtom, useApiQuery, useFetchApi } from "@/lib/node";
import { Button, List, TouchableRipple } from "react-native-paper";
import { Button as BaseButton, Pressable, StyleSheet } from "react-native";
import { useCallback } from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { useAtom } from "jotai";
import { format, formatDate, formatRelative } from "date-fns";

export default function NodeListItem({ node }) {
  const fetchApi = useFetchApi();
  const queryClient = useQueryClient();
  const homeserverUrl = useAtom(homeserverUrlAtom);

  const { data: nodeInfo } = useApiQuery({
    queryKey: ["node", node.id],
    queryFn: async () => fetchApi(`/node/${node.id}`),
    staleTime: 1000,
  });

  const toggleCheck = useCallback(() => {
    (async () => {
      await fetchApi(`/node/${node.id}`, {
        method: "PATCH",
        headers: {
          "Content-Type": "application/json",
        },
        body: JSON.stringify({
          task_status: nodeInfo.task_status === "DONE" ? "TODO" : "DONE",
        }),
      });

      console.log("done!");

      queryClient.invalidateQueries({
        // TODO: Have a more specific one
        queryKey: [],
      });
    })();
  }, [fetchApi, nodeInfo, homeserverUrl]);

  const now = new Date();

  return (
    <List.Item
      title={nodeInfo?.title ?? node.id}
      description={formatRelative(node.last_updated_at, now)}
      left={(props) => {
        if (nodeInfo?.task_status) {
          const icons = {
            TODO: "check-circle-outline",
            DONE: "check-circle",
          };
          return (
            <TouchableRipple
              style={styles.checkButton}
              onPress={(e) => {
                e.stopPropagation();
                e.preventDefault();
                toggleCheck();
              }}
            >
              <List.Icon {...props} icon={icons[nodeInfo.task_status]} />
            </TouchableRipple>
          );
        }

        return <List.Icon {...props} icon="file-outline" />;
      }}
    />
  );
}

const styles = StyleSheet.create({
  checkButton: {
    padding: 0,
  },
  checkButtonLabel: {
    padding: 0,
  },
});
