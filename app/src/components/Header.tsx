import styles from "./Header.module.scss";
import NoteAddIcon from "@mui/icons-material/NoteAdd";
import SearchBar from "./SearchBar";
import { getVersion } from "@tauri-apps/api/app";
import ListIcon from "@mui/icons-material/List";
import { useSetAtom } from "jotai";
import { sidebarExpandedAtom } from "./Sidebar";
import { nodesOpenedAtom } from "../App";
import { useCallback } from "react";

const version = await getVersion();

export default function Header() {
	const setNodesOpened = useSetAtom(nodesOpenedAtom);
	const setSidebarExpanded = useSetAtom(sidebarExpandedAtom);

	const createNewJournalPage = useCallback(() => {
		(async () => {
			const resp = await fetch("http://localhost:5195/node", {
				method: "PUT",
				headers: {
					"Content-Type": "application/json",
				},
				body: JSON.stringify({
					type: "panorama/journal/page",
					extra_data: {
						"panorama/journal/page/content": "",
					},
				}),
			});
			const data = await resp.json();
			setNodesOpened((prev) => [data.node_id, ...prev]);
		})();
	}, [setNodesOpened]);

	return (
		<>
			<div className={styles.Header}>
				<button
					type="button"
					onClick={() => setSidebarExpanded((prev) => !prev)}
				>
					<ListIcon />
				</button>
				<div className={styles.brand}>
					<span className={styles.title}>Panorama</span>
					<span className={styles.version}>v{version}</span>
				</div>
				<button type="button" onClick={createNewJournalPage}>
					<NoteAddIcon />
				</button>
				<SearchBar />
			</div>
			<div className={styles.headerBorder} />
		</>
	);
}
