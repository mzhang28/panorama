import { Component, FC } from "react";
import JournalPage from "../components/nodes/JournalPage";
import Mail from "../components/nodes/Mail";
import { QueryFunctionContext } from "@tanstack/react-query";

export interface RenderProps<D> {
	id: string;
	data: D;
}

export interface NodeDescriptor<D> {
	render: FC<RenderProps<D>>;
	data: {
		type: string;
	} & D;
}

export async function getNode({
	queryKey,
}: QueryFunctionContext): Promise<NodeDescriptor<unknown>> {
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
