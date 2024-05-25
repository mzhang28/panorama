import { useQuery } from "@tanstack/react-query";
import styles from "./NodeDisplay.module.scss";
import ReactTimeAgo from "react-time-ago";
import Markdown from "react-markdown";
import MDEditor, { commands } from "@uiw/react-md-editor";
import { useState } from "react";

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
					<small>ID {id}</small>
				)}
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
		<small>
			Type {data.type} &middot; Last updated{" "}
			<ReactTimeAgo date={data.created_at * 1000} /> &middot; {id}
		</small>
	);
}

function NodeDisplayLoaded({ id, data }) {
	const [value, setValue] = useState(() => data.content);
	const [isEditing, setIsEditing] = useState(() => false);
	return (
		<>
			<details>
				<summary>JSON</summary>
				<pre>{JSON.stringify(data, null, 2)}</pre>
			</details>

			<button type="button" onClick={() => setIsEditing((prev) => !prev)}>
				{isEditing ? "done" : "edit"}
			</button>

			<div className={styles.mdContent} data-color-mode="light">
				{
					isEditing ? (
						<>
							<MDEditor
								data-color-mode="light"
								className={styles.mdEditor}
								value={value}
								onChange={setValue}
							/>
						</>
					) : (
						<>
							<MDEditor.Markdown
								source={value}
								style={{ whiteSpace: "pre-wrap" }}
							/>
						</>
					)
					// <Markdown>{data.content}</Markdown>
				}
			</div>
		</>
	);
}
