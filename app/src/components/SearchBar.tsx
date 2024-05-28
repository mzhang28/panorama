import styles from "./SearchBar.module.scss";
import {
	FloatingFocusManager,
	FloatingOverlay,
	FloatingPortal,
	autoUpdate,
	offset,
	useClick,
	useDismiss,
	useFloating,
	useFocus,
	useInteractions,
} from "@floating-ui/react";
import { useDebounce } from "use-debounce";
import { useEffect, useState } from "react";
import { atom, useAtom, useSetAtom } from "jotai";
import { useNodeControls } from "../App";

const searchQueryAtom = atom("");
const showMenuAtom = atom(false);

export default function SearchBar() {
	const [showMenu, setShowMenu] = useAtom(showMenuAtom);
	const [searchQuery, setSearchQuery] = useAtom(searchQueryAtom);
	const [searchResults, setSearchResults] = useState([]);

	const { refs, context, floatingStyles } = useFloating({
		placement: "bottom-start",
		open: showMenu,
		onOpenChange: setShowMenu,
		whileElementsMounted: autoUpdate,
		middleware: [offset(10)],
	});
	const focus = useFocus(context);
	const { getReferenceProps, getFloatingProps } = useInteractions([
		focus,
		useDismiss(context),
	]);

	useEffect(() => {
		setSearchResults([]);
		const trimmed = searchQuery.trim();
		if (trimmed === "") return;

		(async () => {
			const params = new URLSearchParams();
			params.set("query", trimmed);
			const resp = await fetch(
				`http://localhost:5195/node/search?${params.toString()}`,
			);
			const data = await resp.json();
			setSearchResults(data.results);
		})();
	}, [searchQuery]);

	return (
		<>
			<div>
				<input
					className={styles.entry}
					type="text"
					placeholder="Search..."
					onFocus={() => setShowMenu(true)}
					ref={refs.setReference}
					value={searchQuery}
					onChange={(evt) => setSearchQuery(evt.target.value)}
					{...getReferenceProps()}
				/>
			</div>

			{showMenu && (
				<FloatingPortal>
					<FloatingOverlay>
						<div
							ref={refs.setFloating}
							className={styles.menu}
							style={{ ...floatingStyles }}
							{...getFloatingProps()}
						>
							<SearchMenu results={searchResults} />
						</div>
					</FloatingOverlay>
				</FloatingPortal>
			)}
		</>
	);
}

function SearchMenu({ results }) {
	const setSearchQuery = useSetAtom(searchQueryAtom);
	const setShowMenu = useSetAtom(showMenuAtom);
	const { openNode } = useNodeControls();

	return (
		<div className={styles.searchResults}>
			{results.map((result) => {
				return (
					<button
						type="button"
						key={result.node_id}
						className={styles.searchResult}
						onClick={() => {
							setSearchQuery("");
							setShowMenu(false);
							openNode(result.node_id);
						}}
					>
						<div className={styles.title}>{result.title}</div>
						<div className={styles.subtitle}>{result.content}</div>
					</button>
				);
			})}
		</div>
	);
}
