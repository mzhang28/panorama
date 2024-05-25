import Header from "./components/Header";
import styles from "./App.module.scss";

import "@fontsource/inter";
import "./global.scss";
import { useEffect, useState } from "react";
import NodeDisplay from "./components/NodeDisplay";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import TimeAgo from "javascript-time-ago";
import en from "javascript-time-ago/locale/en";

const queryClient = new QueryClient();

TimeAgo.addDefaultLocale(en);

function App() {
	const [nodesOpened, setNodesOpened] = useState<string[]>(() => []);

	useEffect(() => {
		(async () => {
			console.log("ndoes", nodesOpened);
			if (nodesOpened.length === 0) {
				console.log("Opening today's entry.");
				const resp = await fetch(
					"http://localhost:5195/journal/get_todays_journal_id",
				);
				const data = await resp.json();
				console.log("resp", data);
				setNodesOpened([data.node_id]);
			}
		})();
	}, [nodesOpened]);

	const nodes = nodesOpened.map((nodeId) => (
		<NodeDisplay key={nodeId} id={nodeId} />
	));

	return (
		<QueryClientProvider client={queryClient}>
			<div className={styles.container}>
				<Header />

				<div className={styles.nodeContainer}>{nodes}</div>
			</div>
		</QueryClientProvider>
	);
}

export default App;
