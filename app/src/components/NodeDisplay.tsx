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
			const json = await resp.json();
			return json;
		},
	});
	const { isSuccess, status, data } = query;

	return (
		<div className={styles.container}>
			<div className={styles.header}>
				<small>ID {id}</small>
			</div>
			<div className={styles.body}>
				{isSuccess ? (
					<NodeDisplayLoaded id={id} data={data} />
				) : (
					<>Status: {status}</>
				)}
			</div>
		</div>
	);
}

function NodeDisplayLoaded({ id, data }) {
	return (
		<>
			Node {id}
			<p>{JSON.stringify(data)}</p>
		</>
	);
}
