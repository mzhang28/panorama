import { atom, useAtomValue } from "jotai";
import styles from "./Sidebar.module.scss";
import classNames from "classnames";
import EmailIcon from "@mui/icons-material/Email";
import SettingsIcon from "@mui/icons-material/Settings";

export const sidebarExpandedAtom = atom(false);

export default function Sidebar() {
	const sidebarExpanded = useAtomValue(sidebarExpandedAtom);

	return (
		<div
			className={classNames(
				styles.sidebar,
				sidebarExpanded ? styles.expanded : styles.collapsed,
			)}
		>
			<div className={styles.item}>
				<EmailIcon />
				<span className={styles.label}>Email</span>
			</div>

			<div className="spacer" />

			<div className={styles.item}>
				<SettingsIcon />
				<span className={styles.label}>Settings</span>
			</div>
		</div>
	);
}
