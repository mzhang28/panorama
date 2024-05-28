import Header from "./components/Header";
import styles from "./App.module.scss";

import "@fontsource/inter";
import "@fontsource/inter/700.css";
import "./global.scss";
import "katex/dist/katex.min.css";
import { useEffect, useState } from "react";
import NodeDisplay from "./components/NodeDisplay";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import TimeAgo from "javascript-time-ago";
import en from "javascript-time-ago/locale/en";
import Sidebar from "./components/Sidebar";
import { atom, useAtom, useAtomValue } from "jotai";
import { OrderedMap, OrderedSet } from "immutable";

const queryClient = new QueryClient();

TimeAgo.addDefaultLocale(en);

export const nodesOpenedAtom = atom<OrderedSet<string>>(OrderedSet<string>());

function App() {
	const nodesOpened = useAtomValue(nodesOpenedAtom);
	const { openNode } = useNodeControls();

	// Open today's journal entry if it's not already opened
	useEffect(() => {
		(async () => {
			console.log("ndoes", nodesOpened);
			if (nodesOpened.size === 0) {
				console.log("Opening today's entry.");
				const resp = await fetch(
					"http://localhost:5195/journal/get_todays_journal_id",
				);
				const data = await resp.json();
				console.log("resp", data);
				openNode(data.node_id);
			}
		})();
	}, [nodesOpened, openNode]);

	const nodes = [...nodesOpened.reverse().values()].map((nodeId, idx) => (
		<NodeDisplay idx={idx} key={nodeId} id={nodeId} />
	));

	return (
		<QueryClientProvider client={queryClient}>
			<div className={styles.container}>
				<Header />

				<div className={styles.main}>
					<Sidebar />
					<div className={styles.nodeContainer}>{nodes}</div>
				</div>
			</div>
		</QueryClientProvider>
	);
}

export default App;

export function useNodeControls() {
	const [nodesOpened, setNodesOpened] = useAtom(nodesOpenedAtom);
	return {
		isOpen: (node_id: string) => nodesOpened.has(node_id),
		toggleNode: (node_id: string) => {
			if (nodesOpened.has(node_id)) setNodesOpened(nodesOpened.remove(node_id));
			else setNodesOpened(nodesOpened.remove(node_id).add(node_id));
		},
		openNode: (node_id: string) => {
			setNodesOpened(nodesOpened.remove(node_id).add(node_id));
		},
		closeNode: (node_id: string) => {
			setNodesOpened(nodesOpened.remove(node_id));
		},
	};
}
