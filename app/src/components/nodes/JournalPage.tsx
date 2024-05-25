import { createContext, useCallback, useContext, useState } from "react";
import styles from "./JournalPage.module.scss";
import MDEditor from "@uiw/react-md-editor";
import Markdown from "react-markdown";
import { toMdast } from "hast-util-to-mdast";
import { fromMarkdown } from "mdast-util-from-markdown";
import { toMarkdown } from "mdast-util-to-markdown";

const MDContext = createContext(null);

export default function JournalPage({ id, data }) {
	const [content, setContent] = useState(() => data.content);
	const [isEditing, setIsEditing] = useState(() => false);

	const tree = fromMarkdown(data.content);
	console.log("tree", tree);

	const contextValue = { content, setContent, isEditing, setIsEditing };
	return (
		<>
			<details>
				<summary>JSON</summary>
				<pre>{JSON.stringify(data, null, 2)}</pre>
			</details>

			<div className={styles.mdContent} data-color-mode="light"></div>
		</>
	);
}
