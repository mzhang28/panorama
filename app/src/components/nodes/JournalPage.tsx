import { useEffect, useState } from "react";
import MDEditor from "@uiw/react-md-editor";
import { usePrevious, useDebounce } from "@uidotdev/usehooks";
import { useQueryClient } from "@tanstack/react-query";
import styles from "./JournalPage.module.scss";
import remarkMath from "remark-math";
import rehypeKatex from "rehype-katex";

export interface JournalPageProps {
	id: string;
	data: {
		content: string;
	};
}

export default function JournalPage({ id, data }: JournalPageProps) {
	const queryClient = useQueryClient();
	const [value, setValue] = useState(() => data.content);
	const valueToSave = useDebounce(value, 1000);
	const previous = usePrevious(valueToSave);
	const changed = valueToSave !== previous;

	useEffect(() => {
		if (changed) {
			(async () => {
				console.log("Saving...");
				const resp = await fetch(`http://localhost:5195/node/${id}`, {
					method: "POST",
					headers: {
						"Content-Type": "application/json",
					},
					body: JSON.stringify({
						extra_data: {
							"panorama/journal/page/content": valueToSave,
						},
					}),
				});
				const data = await resp.text();
				console.log("result", data);

				queryClient.invalidateQueries({ queryKey: ["fetchNode", id] });
			})();
		}
	}, [id, changed, valueToSave, queryClient]);

	return (
		<div data-color-mode="light" className={styles.container}>
			<MDEditor
				value={value}
				className={styles.mdEditor}
				onChange={(newValue) => newValue && setValue(newValue)}
				preview="preview"
				visibleDragbar={false}
				previewOptions={{
					remarkPlugins: [remarkMath],
					rehypePlugins: [rehypeKatex],
				}}
			/>
		</div>
	);
}
