import { useQuery } from "@tanstack/react-query";
import styles from "./NodeDisplay.module.scss";
import ReactTimeAgo from "react-time-ago";
import { getNode } from "../lib/getNode";
import FirstPageIcon from "@mui/icons-material/FirstPage";
import MoreVertIcon from "@mui/icons-material/MoreVert";
import CloseIcon from "@mui/icons-material/Close";
import { useNodeControls } from "../App";

export interface NodeDisplayProps {
	id: string;
	idx?: number | undefined;
}

export default function NodeDisplay({ id, idx }: NodeDisplayProps) {
	const query = useQuery({
		queryKey: ["fetchNode", id],
		queryFn: getNode,
	});

	const { isSuccess, status, data: nodeDescriptor } = query;

	let Component = undefined;
	let data = undefined;
	if (isSuccess) {
		Component = nodeDescriptor.render;
		data = nodeDescriptor.data;
	}

	return (
		<div className={styles.container}>
			<div className={styles.header}>
				{isSuccess ? (
					<NodeDisplayHeaderLoaded idx={idx} id={id} data={data} />
				) : (
					<>
						ID {id} ({status})
					</>
				)}
			</div>

			<div className={styles.body}>
				{Component && <Component id={id} data={data} />}
			</div>

			<div className={styles.footer}>{id}</div>
		</div>
	);
}

function NodeDisplayHeaderLoaded({ idx, id, data }) {
	const { openNode, closeNode } = useNodeControls();

	return (
		<>
			{idx === 0 || (
				<button
					type="button"
					onClick={() => openNode(id)}
					title="Move node to the left"
				>
					<FirstPageIcon fontSize="inherit" />
				</button>
			)}
			<span>
				Type {data.type}{" "}
				{data.created_at && (
					<>
						&middot; Last updated <ReactTimeAgo date={data.updated_at * 1000} />
					</>
				)}
			</span>
			<div className="spacer" />

			<button type="button">
				<MoreVertIcon fontSize="inherit" />
			</button>

			<button
				type="button"
				className={styles.closeButton}
				onClick={() => closeNode(id)}
			>
				<CloseIcon fontSize="inherit" />
			</button>
		</>
	);
}
