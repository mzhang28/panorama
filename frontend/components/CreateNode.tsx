import {
  ArrowRightIcon,
  Check,
  Circle,
  CircleSmall,
  Icon,
  X,
} from "lucide-react";
import { Formik } from "formik";
import { useState } from "react";
import { useCallback } from "react";
import { cn } from "@/lib/utils";
import { useQueryClient } from "@tanstack/react-query";

export default function CreateNode() {
  const queryClient = useQueryClient();

  const submitForm = useCallback(
    async (values, { setSubmitting, resetForm }) => {
      let values2 = values;

      if (values2.task_status !== null) {
        values2 = { ...values2, content: null, title: values2.content };
        queryClient.invalidateQueries({ queryKey: [] });
      }

      setSubmitting(true);
      try {
        await fetch("/api/node", {
          method: "PUT",
          headers: {
            "Content-Type": "application/json",
          },
          body: JSON.stringify(values2),
        });
      } catch (error) {
        console.error(error);
      } finally {
        setSubmitting(false);
        resetForm();
      }
    },
    [],
  );

  return (
    <Formik
      initialValues={{ title: "Note", content: "", task_status: null }}
      onSubmit={submitForm}
    >
      {({
        values,
        handleSubmit,
        isSubmitting,
        handleChange,
        handleBlur,
        setValues,
      }) => (
        <form onSubmit={handleSubmit}>
          <div className="flex flex-col gap-2">
            <textarea
              className="border-1 outline-none p-2 bg-slate-50"
              placeholder="What's new?"
              name="content"
              value={values.content}
              onBlur={handleBlur}
              onChange={handleChange}
            />

            <div className="flex justify-end">
              <button
                type="button"
                className={cn("flex px-3 py-2 gap-1")}
                onClick={() =>
                  setValues({
                    ...values,
                    task_status: values.task_status === null ? "TODO" : null,
                  })
                }
              >
                {values.task_status === null ? (
                  <CircleSmall />
                ) : (
                  <CircleSmall fill="bg-slate-500" />
                )}{" "}
                is task?
              </button>

              <button
                type="submit"
                className="flex bg-slate-600 text-white px-3 py-2"
                disabled={isSubmitting}
              >
                Save to panorama <ArrowRightIcon />
              </button>
            </div>
          </div>
        </form>
      )}
    </Formik>
  );
}
