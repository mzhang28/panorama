import styles from "./Header.module.scss";
import SearchBar from "./SearchBar";

export default function Header() {
	return (
		<div className={styles.Header}>
			<span>Panorama</span>
			<SearchBar />
		</div>
	);
}
