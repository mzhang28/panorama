import { createFileRoute } from "@tanstack/react-router";
import FitnessPage from "@/pages/fitness";

export const Route = createFileRoute("/fitness")({
  component: FitnessPage,
});
