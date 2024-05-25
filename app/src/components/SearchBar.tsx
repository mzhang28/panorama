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
import { useState } from "react";

export default function SearchBar() {
	const [showMenu, setShowMenu] = useState(() => false);

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

	return (
		<>
			<div>
				<input
					className={styles.entry}
					type="text"
					placeholder="Search..."
					onFocus={() => setShowMenu(true)}
					ref={refs.setReference}
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
							<SearchMenu />
						</div>
					</FloatingOverlay>
				</FloatingPortal>
			)}
		</>
	);
}

function SearchMenu() {
	return <>Search suggestions...</>;
}
