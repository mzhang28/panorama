export interface NodeInfo {
	fields: Partial<NodeFields>;
}

export const JOURNAL_PAGE_CONTENT_FIELD_NAME = "panorama/journal/page/content";
export const JOURNAL_PAGE_TITLE_FIELD_NAME = "panorama/journal/page/title";

export interface NodeFields {
	[JOURNAL_PAGE_CONTENT_FIELD_NAME]: string;
	[JOURNAL_PAGE_TITLE_FIELD_NAME]: string;
}
