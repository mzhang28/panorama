export interface NodeContainerProps {
  id: string;
}

export default function NodeContainer({ id }: NodeContainerProps) {
  return <>{id}</>;
}
