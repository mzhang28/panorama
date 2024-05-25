import { useQuery } from "@tanstack/react-query";
import styles from "./NodeDisplay.module.scss";
import ReactTimeAgo from "react-time-ago";
import JournalPage from "./nodes/JournalPage";

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
				{isSuccess ? (
					<NodeDisplayHeaderLoaded id={id} data={data} />
				) : (
					<>ID {id}</>
				)}
			</div>
			<div className={styles.title}>
				{data?.title ?? <span className={styles.untitled}>(untitled)</span>}
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

function NodeDisplayHeaderLoaded({ id, data }) {
	return (
		<>
			Type {data.type} &middot; Last updated{" "}
			<ReactTimeAgo date={data.created_at * 1000} /> &middot; {id}
		</>
	);
}

function NodeDisplayLoaded({ id, data }) {
	switch (data.type) {
		case "panorama/journal/page":
			return <JournalPage id={id} data={data} />;

		default:
			return (
				<>
					Don't know how to render node of type <code>{data.type}</code>
				</>
			);
	}
}
