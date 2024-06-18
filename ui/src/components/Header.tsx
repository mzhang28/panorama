import styles from "./Header.module.scss";
import NoteAddIcon from "@mui/icons-material/NoteAdd";
import SearchBar from "./SearchBar";
import ListIcon from "@mui/icons-material/List";
import ArrowDropDownIcon from "@mui/icons-material/ArrowDropDown";
import { useSetAtom } from "jotai";
import { sidebarExpandedAtom } from "./Sidebar";
import { useNodeControls } from "../App";
import { useCallback, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { getVersion } from "@tauri-apps/api/app";
import {
	FloatingOverlay,
	FloatingPortal,
	autoUpdate,
	offset,
	useDismiss,
	useFloating,
	useFocus,
	useInteractions,
} from "@floating-ui/react";

export default function Header() {
	const { openNode } = useNodeControls();
	const setSidebarExpanded = useSetAtom(sidebarExpandedAtom);
	const versionData = useQuery({
		queryKey: ["appVersion"],
		queryFn: getVersion,
	});
	const { data: version } = versionData;
	console.log("version", version);

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
				<div>
					<button type="button" onClick={createNewJournalPage}>
						<NoteAddIcon fontSize="inherit" />
					</button>
					<NewNodeButton />
				</div>
				<SearchBar />
			</div>
			<div className={styles.headerBorder} />
		</>
	);
}

function NewNodeButton() {
	const [showMenu, setShowMenu] = useState(false);
	const { refs, context, floatingStyles } = useFloating({
		placement: "bottom-start",
		open: showMenu,
		onOpenChange: setShowMenu,
		whileElementsMounted: autoUpdate,
		// middleware: [offset(10)],
	});
	const focus = useFocus(context);
	const { getReferenceProps, getFloatingProps } = useInteractions([
		focus,
		useDismiss(context),
	]);

	return (
		<>
			<button
				type="button"
				onClick={() => setShowMenu((p) => !p)}
				ref={refs.setReference}
			>
				<ArrowDropDownIcon fontSize="inherit" />
			</button>

			{showMenu && (
				<FloatingPortal>
					<FloatingOverlay>
						<div
							ref={refs.setFloating}
							style={{ ...floatingStyles }}
							{...getFloatingProps()}
						>
							<NewNodeMenu />
						</div>
					</FloatingOverlay>
				</FloatingPortal>
			)}
		</>
	);
}

function NewNodeMenu() {
	return (
		<div className={styles.newNodeMenu}>
			<ul>
				<li>
					<button type="button">Journal Page</button>
				</li>
				<li>
					<button type="button">Media resource</button>
				</li>
			</ul>
		</div>
	);
}
