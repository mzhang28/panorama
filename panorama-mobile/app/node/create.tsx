import { SafeAreaView, View, StyleSheet } from "react-native";
import { Button, Switch, Text, TextInput } from "react-native-paper";
import { Formik } from "formik";
import { useCallback } from "react";
import { useRouter } from "expo-router";
import { useFetchApi } from "@/lib/node";
import { useQueryClient } from "@tanstack/react-query";
import {
  MarkdownTextInput,
  parseExpensiMark,
} from "@expensify/react-native-live-markdown";

export default function CreateNode() {
  const fetchApi = useFetchApi();
  const router = useRouter();
  const queryClient = useQueryClient();

  const submitForm = useCallback(
    (values, { setSubmitting }) => {
      setSubmitting(true);
      (async () => {
        console.log("SHIET", values);
        await fetchApi(`/node`, {
          method: "PUT",
          headers: {
            "Content-Type": "application/json",
          },
          body: JSON.stringify(
            values.is_task
              ? {
                  title: values.content,
                  task_status: "TODO",
                }
              : {
                  content: values.content,
                },
          ),
        });

        queryClient.invalidateQueries({ queryKey: [] });

        router.back();
      })();
    },
    [fetchApi],
  );

  return (
    <SafeAreaView style={styles.safeAreaView}>
      <View style={styles.container}>
        <Text>Create Node</Text>

        <Formik
          initialValues={{ content: "", is_task: false }}
          onSubmit={submitForm}
        >
          {({
            values,
            handleChange,
            handleBlur,
            handleSubmit,
            setValues,
            errors,
          }) => (
            <View style={styles.form}>
              <MarkdownTextInput
                value={values.content}
                onChangeText={handleChange("content")}
                onBlur={handleBlur("content")}
                parser={parseExpensiMark}
                style={styles.contentEditor}
                multiline
              />
              <View style={styles.switchContainer}>
                <Text>Is this a task?</Text>
                <Switch
                  value={values.is_task}
                  onValueChange={(v) => {
                    setValues({ ...values, is_task: v });
                  }}
                  // onBlur={handleBlur("task_status")}
                />
              </View>
              {/* <Text>Errors: {JSON.stringify(errors)}</Text> */}
              <Button
                mode="elevated"
                onPress={() => handleSubmit()}
                disabled={Object.keys(errors).length > 0}
              >
                Create
              </Button>
            </View>
          )}
        </Formik>
      </View>
    </SafeAreaView>
  );
}

const styles = StyleSheet.create({
  safeAreaView: { flex: 1 },
  container: { padding: 16 },
  form: { display: "flex", flexDirection: "column", gap: 16 },
  fab: {
    position: "absolute",
    bottom: 16,
    right: 16,
  },
  switchContainer: {
    display: "flex",
    flexDirection: "row",
    alignItems: "center",
  },
  contentEditor: {
    height: 200,
    borderWidth: 1,
    backgroundColor: "#fff",
    borderColor: "#ccc",
    borderRadius: 4,
    padding: 8,
    verticalAlign: "top",
  },
});
