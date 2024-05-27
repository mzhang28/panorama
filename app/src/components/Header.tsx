import styles from "./Header.module.scss";
import NoteAddIcon from "@mui/icons-material/NoteAdd";
import SearchBar from "./SearchBar";
import { getVersion } from "@tauri-apps/api/app";
import ListIcon from "@mui/icons-material/List";
import { useSetAtom } from "jotai";
import { sidebarExpandedAtom } from "./Sidebar";

const version = await getVersion();

export default function Header() {
	const setSidebarExpanded = useSetAtom(sidebarExpandedAtom);
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
				<button type="button">
					<NoteAddIcon />
				</button>
				<SearchBar />
			</div>
			<div className={styles.headerBorder} />
		</>
	);
}
