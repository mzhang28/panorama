import { Component, FC } from "react";
import JournalPage from "../components/nodes/JournalPage";
import Mail from "../components/nodes/Mail";
import { QueryFunctionContext } from "@tanstack/react-query";

export interface RenderProps {
	id: string;
	data: any;
}

export interface NodeDescriptor {
	render: FC<RenderProps>;
	data: {
		type: string;
	} & any;
}

export async function getNode({
	queryKey,
}: QueryFunctionContext): Promise<NodeDescriptor> {
	const [, node_id] = queryKey;
	switch (node_id) {
		case "panorama/mail":
			return { data: { type: "panorama/mail" }, render: Mail };
		default: {
			const resp = await fetch(`http://localhost:5195/node/${node_id}`);
			const data = await resp.json();
			return {
				data: { ...data, type: "panorama/journal/page" },
				render: JournalPage,
			};
		}
	}
}
