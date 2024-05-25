import styles from "./Header.module.scss";

export default function Header() {
	return (
		<div className={styles.Header}>
			<span>Panorama</span>
			<input type="text" placeholder="Search..." />
		</div>
	);
}
