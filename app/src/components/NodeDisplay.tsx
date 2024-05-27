import { useQuery } from "@tanstack/react-query";
import styles from "./NodeDisplay.module.scss";
import ReactTimeAgo from "react-time-ago";
import JournalPage from "./nodes/JournalPage";
import { useCallback, useEffect, useState } from "react";

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

	const [isEditingTitle, setIsEditingTitle] = useState(false);
	const [title, setTitle] = useState(() =>
		isSuccess && data ? data.title : undefined,
	);

	useEffect(() => {
		if (data) {
			setTitle(data.title);
		}
	}, [data]);

	const saveChangedTitle = useCallback(() => {
		(async () => {
			const resp = await fetch(`http://localhost:5195/node/${id}`, {
				method: "POST",
				headers: {
					"Content-Type": "application/json",
				},
				body: JSON.stringify({ title: title }),
			});
			setIsEditingTitle(false);
		})();
	}, [title, id]);

	return (
		<div className={styles.container}>
			<div className={styles.header}>
				{isSuccess ? (
					<NodeDisplayHeaderLoaded id={id} data={data} />
				) : (
					<>ID {id}</>
				)}
			</div>
			{isEditingTitle ? (
				<form
					onSubmit={(evt) => {
						evt.preventDefault();
						saveChangedTitle();
					}}
				>
					<input
						className={styles.title}
						type="text"
						value={title}
						onChange={(evt) => setTitle(evt.target.value)}
						onBlur={() => saveChangedTitle()}
						// biome-ignore lint/a11y/noAutofocus: <explanation>
						autoFocus
					/>
				</form>
			) : (
				<div
					className={styles.title}
					onDoubleClick={() => setIsEditingTitle(true)}
				>
					{title ?? <span className={styles.untitled}>(untitled)</span>}
				</div>
			)}
			<div className={styles.body}>
				{isSuccess ? (
					<NodeDisplayLoaded id={id} data={data} />
				) : (
					<>Status: {status}</>
				)}
			</div>

			<div className={styles.footer}>{id}</div>
		</div>
	);
}

function NodeDisplayHeaderLoaded({ id, data }) {
	return (
		<>
			Type {data.type} &middot; Last updated{" "}
			<ReactTimeAgo date={data.created_at * 1000} />
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
