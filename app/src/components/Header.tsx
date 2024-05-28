import styles from "./Header.module.scss";
import NoteAddIcon from "@mui/icons-material/NoteAdd";
import SearchBar from "./SearchBar";
import { getVersion } from "@tauri-apps/api/app";
import ListIcon from "@mui/icons-material/List";
import { useSetAtom } from "jotai";
import { sidebarExpandedAtom } from "./Sidebar";
import { useNodeControls } from "../App";
import { useCallback } from "react";
import { useQuery } from "@tanstack/react-query";

export default function Header() {
	const { openNode } = useNodeControls();
	const setSidebarExpanded = useSetAtom(sidebarExpandedAtom);
	const versionData = useQuery({
		queryKey: ["appVersion"],
		queryFn: getVersion,
	});
	const { data: version } = versionData;

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
			openNode(data.node_id);
		})();
	}, [openNode]);

	return (
		<>
			<div className={styles.Header}>
				<button
					type="button"
					onClick={() => setSidebarExpanded((prev) => !prev)}
				>
					<ListIcon fontSize="inherit" />
				</button>
				<div className={styles.brand}>
					<span className={styles.title}>Panorama</span>
					<span className={styles.version}>v{version}</span>
				</div>
				<button type="button" onClick={createNewJournalPage}>
					<NoteAddIcon fontSize="inherit" />
				</button>
				<SearchBar />
			</div>
			<div className={styles.headerBorder} />
		</>
	);
}
