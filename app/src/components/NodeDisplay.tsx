import { useQuery } from "@tanstack/react-query";
import styles from "./NodeDisplay.module.scss";

export interface NodeDisplayProps {
	id: string;
}

export default function NodeDisplay({ id }: NodeDisplayProps) {
	const query = useQuery({
		queryKey: ["fetchNode", id],
		queryFn: async () => {
			const resp = await fetch(`http://localhost:5195/node/${id}`);
			console.log("id", resp);
			return "helloge";
		},
	});

	return (
		<div className={styles.container}>
			Node {id}
			<p>{JSON.stringify(query)}</p>
		</div>
	);
}
