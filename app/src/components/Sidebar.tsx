import { atom, useAtomValue } from "jotai";
import styles from "./Sidebar.module.scss";
import classNames from "classnames";
import EmailIcon from "@mui/icons-material/Email";
import SettingsIcon from "@mui/icons-material/Settings";
import { useNodeControls } from "../App";

export const sidebarExpandedAtom = atom(false);

export default function Sidebar() {
	const sidebarExpanded = useAtomValue(sidebarExpandedAtom);
	const { toggleNode, isOpen } = useNodeControls();

	return (
		<div
			className={classNames(
				styles.sidebar,
				sidebarExpanded ? styles.expanded : styles.collapsed,
			)}
		>
			<button
				type="button"
				className={classNames(
					styles.item,
					isOpen("panorama/mail") && styles.active,
				)}
				onClick={() => toggleNode("panorama/mail")}
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
