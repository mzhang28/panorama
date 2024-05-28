import { useCallback, useEffect, useRef, useState } from "react";
import MDEditor, { PreviewType } from "@uiw/react-md-editor";
import { usePrevious } from "@uidotdev/usehooks";
import { useQueryClient } from "@tanstack/react-query";
import styles from "./JournalPage.module.scss";
import remarkMath from "remark-math";
import rehypeKatex from "rehype-katex";
import { parse as parseDate, format as formatDate } from "date-fns";
import { useDebounce } from "use-debounce";

export interface JournalPageProps {
	id: string;
	data: {
		day?: string;
		title?: string;
		content: string;
	};
}

export default function JournalPage({ id, data }: JournalPageProps) {
	const { day } = data;
	const queryClient = useQueryClient();
	const [value, setValue] = useState(() => data.content);
	const [valueToSave] = useDebounce(value, 1000, {
		leading: true,
		trailing: true,
	});
	const previous = usePrevious(valueToSave);
	const changed = valueToSave !== previous;
	const [mode, setMode] = useState<PreviewType>("preview");
	const [title, setTitle] = useState(() => data.title);
	const [isEditingTitle, setIsEditingTitle] = useState(false);

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
		<>
			{isEditingTitle ? (
				<form
					className={styles.titleEditorForm}
					onSubmit={(evt) => {
						evt.preventDefault();
						saveChangedTitle();
					}}
				>
					<input
						className={styles.titleEditor}
						type="text"
						value={title}
						onChange={(evt) => {
							let newTitle = evt.target.value;
							if (newTitle.trim().length === 0) newTitle = null;
							setTitle(newTitle);
						}}
						onBlur={() => saveChangedTitle()}
						// biome-ignore lint/a11y/noAutofocus: <explanation>
						autoFocus
					/>
				</form>
			) : (
				<div className={styles.titleContainer}>
					<div
						className={styles.title}
						onDoubleClick={() => setIsEditingTitle(true)}
					>
						{title ?? <span className={styles.untitled}>(untitled)</span>}
					</div>
				</div>
			)}
			<div data-color-mode="light" className={styles.container}>
				{day && <DayIndicator day={day} />}

				<MDEditor
					value={value}
					className={styles.mdEditor}
					onChange={(newValue) => newValue !== undefined && setValue(newValue)}
					preview={mode}
					visibleDragbar={false}
					onDoubleClick={() => setMode("live")}
					previewOptions={{
						remarkPlugins: [remarkMath],
						rehypePlugins: [rehypeKatex],
					}}
				/>
			</div>
		</>
	);
}

function DayIndicator({ day }) {
	const parsedDate = parseDate(day, "yyyy-MM-dd", new Date());
	const formattedDate = formatDate(parsedDate, "PPPP");
	return (
		<div className={styles.dayIndicator}>
			Journal entry for <b>{formattedDate}</b>
		</div>
	);
}
