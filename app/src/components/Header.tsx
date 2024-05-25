import styles from "./Header.module.scss";
import NoteAddIcon from "@mui/icons-material/NoteAdd";
import SearchBar from "./SearchBar";

export default function Header() {
	return (
		<div className={styles.Header}>
			<span>Panorama</span>
			<button type="button">
				<NoteAddIcon />
			</button>
			<SearchBar />
		</div>
	);
}
