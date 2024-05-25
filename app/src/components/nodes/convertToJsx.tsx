import { ReactNode } from "react";
import { Nodes as MdastNodes } from "mdast";

export function convertToJsx(tree: MdastNodes): ReactNode {
	console.log("tree", tree);

	switch (tree.type) {
	}
}
