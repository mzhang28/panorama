import { createFileRoute } from "@tanstack/react-router";

export const Route = createFileRoute("/")({
  component: Index,
});

function Index() {
  return (
    <div className="p-2">
      <h3>Welcome Home!</h3>
      <iframe
        src="panorama-static://journal/"
        className="flex-grow w-full border-none"
        title="Journal App"
      />
    </div>
  );
}
