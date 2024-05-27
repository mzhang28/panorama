import { atom, useAtomValue } from "jotai";
import styles from "./Sidebar.module.scss";
import classNames from "classnames";
import EmailIcon from "@mui/icons-material/Email";
import SettingsIcon from "@mui/icons-material/Settings";
import { useOpenNode } from "../App";

export const sidebarExpandedAtom = atom(false);

export default function Sidebar() {
	const sidebarExpanded = useAtomValue(sidebarExpandedAtom);
	const openNode = useOpenNode();

	return (
		<div
			className={classNames(
				styles.sidebar,
				sidebarExpanded ? styles.expanded : styles.collapsed,
			)}
		>
			<button
				type="button"
				className={styles.item}
				onClick={() => openNode("panorama/mail")}
			>
				<EmailIcon />
				<span className={styles.label}>Email</span>
			</button>

			<div className="spacer" />

			<div className={styles.item}>
				<SettingsIcon />
				<span className={styles.label}>Settings</span>
			</div>
		</div>
	);
}
