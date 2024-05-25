import {
	ReactNode,
	createContext,
	useCallback,
	useContext,
	useEffect,
	useRef,
	useState,
} from "react";
import { Fragment, jsx, jsxs } from "react/jsx-runtime";
import styles from "./JournalPage.module.scss";
import MDEditor from "@uiw/react-md-editor";
import Markdown from "react-markdown";
import { toMdast } from "hast-util-to-mdast";
import { Node as MdastNode } from "mdast";
import { fromMarkdown } from "mdast-util-from-markdown";
import { toMarkdown } from "mdast-util-to-markdown";
import { toJsxRuntime } from "hast-util-to-jsx-runtime";
import remarkRehype from "remark-rehype";
import { VFile } from "vfile";
import { common } from "@mui/material/colors";
import classNames from "classnames";

interface MDContextValue {
	isEditing: boolean;
}

// biome-ignore lint/style/noNonNullAssertion: <explanation>
const MDContext = createContext<MDContextValue>(null!);

const emptyContent = { type: "root", children: [] };

export default function JournalPage({ id, data }) {
	const [content, setContent] = useState(() => data.content);
	const [isEditing, setIsEditing] = useState(() => false);
	const [currentlyFocused, setCurrentlyFocused] = useState<string | undefined>(
		() => undefined,
	);

	useEffect(() => {
		if (content === null) {
			setContent(() => ({
				type: "root",
				children: [
					{ type: "paragraph", children: [{ type: "text", value: "" }] },
				],
			}));
			setCurrentlyFocused(".children[0]");
			setIsEditing(true);
		}
	}, [content]);

	const contextValue = { content, setContent, isEditing, setIsEditing };

	const jsxContent = convertToJsx(content, { currentlyFocused });

	return (
		<>
			<details>
				<summary>JSON</summary>
				<pre>{JSON.stringify(data, null, 2)}</pre>
			</details>

			<div className={styles.mdContent} data-color-mode="light">
				<MDContext.Provider value={contextValue}>
					{jsxContent}
				</MDContext.Provider>

				<pre>{JSON.stringify(content, null, 2)}</pre>
			</div>
		</>
	);
}

interface ConvertToJsxOpts {
	currentlyFocused?: string | undefined;
	parent?: MdastNode | undefined;
}

function convertToJsx(
	tree: MdastNode,
	opts?: ConvertToJsxOpts | undefined,
): ReactNode {
	console.log("tree", tree);

	if (tree === null) return;

	const commonProps = {
		node: tree,
		parent: opts?.parent,
	};

	switch (tree.type) {
		case "root":
			return tree.children.map((child) =>
				convertToJsx(child, { parent: tree }),
			);

		case "paragraph":
			return <Paragraph {...commonProps} />;

		default:
			throw new Error(`unhandled ${tree.type}`);
	}
}

function Paragraph({ ...args }) {
	// const { isEditing } = useContext(MDContext);
	const [isEditing, setIsEditing] = useState(() => false);
	const [localValue, setLocalValue] = useState(null);

	const onDoubleClick = useCallback(() => {
		if (!isEditing) {
			setIsEditing(true);
		}
	}, [isEditing]);

	const save = useCallback(() => {
		console.log("saving!", localValue);
	});

	const onPaste = useCallback((evt) => {
		console.log("pasted");
	}, []);

	return (
		<div>
			<div
				className={classNames(styles.block, isEditing && styles.isEditing)}
				contentEditable={isEditing}
				onDoubleClick={onDoubleClick}
				onPaste={onPaste}
				onBlur={save}
			>
				<br />
			</div>
		</div>
	);
}
