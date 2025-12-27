export default function JournalPage() {
  return (
    <div className="w-full h-full flex flex-col">
      <div className="p-4 border-b">
        <h2 className="text-xl font-bold">Journal</h2>
      </div>
      <iframe
        src="panorama-static://journal/"
        className="grow w-full border-none"
        title="Journal App"
      />
    </div>
  );
}
