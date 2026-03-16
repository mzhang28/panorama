export default function FitnessPage() {
  return (
    <div className="w-full h-full flex flex-col">
      <div className="p-4 border-b">
        <h2 className="text-xl font-bold">Fitness Tracker</h2>
      </div>
      <iframe
        src="panorama-static://fitness/"
        className="grow w-full border-none"
        title="Fitness App"
      />
    </div>
  );
}
