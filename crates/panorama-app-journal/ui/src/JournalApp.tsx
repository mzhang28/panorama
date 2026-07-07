/// <reference types="vite/client" />

import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useRef, useState } from "react";

import {
  api,
  type Block,
  type BlockNode,
  fBool,
  fJson,
  fStr,
  normalizeBlock,
} from "./api";
import { GraphView } from "./GraphView";
import { TiptapEditor } from "./TiptapEditor";

async function listPages(): Promise<Block[]> {
  const data = await api("pages");
  return (Array.isArray(data) ? data : (data?.rows ?? [])).map(normalizeBlock);
}

async function getBlock(id: string): Promise<Block> {
  return normalizeBlock(await api(`blocks/${id}`));
}

async function getChildren(id: string): Promise<Block[]> {
  const data = await api(`blocks/${id}/children`);
  return (Array.isArray(data) ? data : (data?.rows ?? [])).map(normalizeBlock);
}

async function getTodayPage(): Promise<Block> {
  return normalizeBlock(await api("pages/today"));
}

async function getBacklinks(pageId: string): Promise<Block[]> {
  const data = await api(`pages/${pageId}/backlinks`);
  return (Array.isArray(data) ? data : (data?.rows ?? [])).map(normalizeBlock);
}

async function renderMarkdown(content: string): Promise<string> {
  try {
    const data = await api("blocks/render", "POST", { content });
    return data.html ?? "";
  } catch {
    return "";
  }
}

// ── Shared UnoCSS strings ─────────────────────────────────────────────────────

const sidebarLink =
  "flex items-center gap-[var(--space-2)] w-full text-left text-[13px] " +
  "px-[var(--space-2)] py-[var(--space-1)] rounded-[var(--radius-sm)] " +
  "border-none bg-transparent text-[var(--text)] cursor-pointer " +
  "hover:bg-[var(--bg-hover)]";

const sidebarLinkActive = "!bg-[var(--accent)] !text-[var(--accent-text)]";

const iconBtnSm =
  "bg-transparent border-none cursor-pointer text-xs " +
  "px-[var(--space-1)] py-0 min-h-0 leading-none";

// ── Journal App ───────────────────────────────────────────────────────────────

const TODAY = new Date().toISOString().slice(0, 10);

export function JournalApp({
  pluginId = "io.mzhang.panorama.journal",
  subpath = "",
  navigate = (path: string) => {},
}: {
  pluginId?: string;
  subpath?: string;
  navigate?: (path: string) => void;
}) {
  const queryClient = useQueryClient();

  const showGraph = subpath === "graph" || subpath === "/graph";
  const isNewRoute = subpath === "new" || subpath === "/new";
  const isTodayRoute =
    subpath === "today" ||
    subpath === "/today" ||
    subpath === "" ||
    subpath === "/";
  const pageIdMatch = subpath.match(/^\/?page\/(.+)$/);
  const selectedPageId = pageIdMatch ? pageIdMatch[1] : null;

  const [editingBlockId, setEditingBlockId] = useState<string | null>(null);
  const [showBacklinks, setShowBacklinks] = useState(false);

  const { data: pages = [], isLoading: pagesLoading } = useQuery({
    queryKey: ["journal-pages"],
    queryFn: listPages,
    refetchInterval: 15_000,
  });

  const { data: todayPage } = useQuery({
    queryKey: ["journal-today"],
    queryFn: getTodayPage,
  });

  const { data: selectedPage } = useQuery({
    queryKey: ["journal-page", selectedPageId],
    queryFn: () => (selectedPageId ? getBlock(selectedPageId) : null),
    enabled: !!selectedPageId,
  });

  const { data: pageChildren = [] } = useQuery({
    queryKey: ["journal-children", selectedPageId],
    queryFn: () => (selectedPageId ? getChildren(selectedPageId) : []),
    enabled: !!selectedPageId,
  });

  const { data: backlinks = [] } = useQuery({
    queryKey: ["journal-backlinks", selectedPageId],
    queryFn: () => (selectedPageId ? getBacklinks(selectedPageId) : []),
    enabled: !!selectedPageId && showBacklinks,
  });

  const createBlock = useMutation({
    mutationFn: (body: any) => api("blocks", "POST", body),
    onSuccess: (data: any, vars: any) => {
      const block = normalizeBlock(data);
      if (block.id) queryClient.setQueryData(["journal-page", block.id], block);
      queryClient.invalidateQueries({ queryKey: ["journal-pages"] });
      queryClient.invalidateQueries({ queryKey: ["journal-children"] });
      queryClient.setQueryData(["journal-pages"], (old: any) => {
        const oldPages = Array.isArray(old) ? old : (old?.rows ?? []);
        return [block, ...oldPages];
      });
      if (!vars.parent_id && block.id) navigate(`page/${block.id}`);
    },
  });

  const navigateRef = useRef(navigate);
  navigateRef.current = navigate;

  useEffect(() => {
    if (isTodayRoute && todayPage?.id) {
      navigateRef.current(`page/${todayPage.id}`);
    }
  }, [isTodayRoute, todayPage]);

  const updateBlock = useMutation({
    mutationFn: ({ id, ...body }: any) => api(`blocks/${id}`, "PUT", body),
    onSuccess: (data: any) => {
      const block = normalizeBlock(data);
      if (block.id) queryClient.setQueryData(["journal-page", block.id], block);
      queryClient.invalidateQueries({ queryKey: ["journal-pages"] });
      queryClient.invalidateQueries({ queryKey: ["journal-children"] });
      setEditingBlockId(null);
    },
  });

  const deleteBlock = useMutation({
    mutationFn: (id: string) => api(`blocks/${id}`, "DELETE"),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["journal-pages"] });
      queryClient.invalidateQueries({ queryKey: ["journal-page"] });
    },
  });

  const activePage = selectedPageId ? selectedPage : null;

  return (
    <div className="journal-layout flex h-[calc(100vh-100px)] gap-0">
      {/* ── Sidebar ──────────────────────────────────────────────────── */}
      <aside
        data-testid="journal-sidebar"
        className="journal-sidebar w-[220px] min-w-[180px] border-r border-[var(--border)] p-[var(--space-3)] overflow-y-auto bg-[var(--bg-card)]"
      >
        <div className="journal-sidebar-section flex flex-col gap-[var(--space-2)] mb-[var(--space-4)]">
          <button
            data-testid="journal-today-btn"
            className={`journal-today-btn ${sidebarLink}`}
            onClick={() => navigate("today")}
          >
            📅 Today
          </button>
          <button
            data-testid="journal-new-page-btn"
            className={`journal-new-page-btn ${sidebarLink}`}
            onClick={() => navigate("new")}
          >
            + New Page
          </button>
          <button
            className={`journal-graph-btn ${sidebarLink} ${showGraph ? sidebarLinkActive : ""}`}
            onClick={() => navigate("graph")}
          >
            🕸️ Graph View
          </button>
        </div>

        <h3 className="journal-sidebar-heading text-[11px] uppercase text-[var(--text-muted)] mb-[var(--space-2)] tracking-wide font-semibold">
          All Pages
        </h3>
        {pagesLoading ? (
          <p className="text-[var(--text-muted)] text-[13px]">Loading...</p>
        ) : pages.length === 0 ? (
          <p className="text-[var(--text-muted)] text-[13px]">No pages yet</p>
        ) : (
          <div
            data-testid="journal-page-list"
            className="journal-page-list flex flex-col gap-0.5"
          >
            {pages.map((p) => {
              const title = fStr(p, "system:node_title") || "Untitled";
              const day = fStr(p, "journal:journal_day");
              const isDeleted = fBool(p, "journal:deleted");
              const active = selectedPageId === normalizeBlock(p).id;
              return (
                <button
                  key={normalizeBlock(p).id || title}
                  className={`journal-page-link ${sidebarLink}${active ? ` active ${sidebarLinkActive}` : ""}${isDeleted ? " deleted opacity-50 line-through" : ""}`}
                  onClick={() => {
                    navigate(`page/${normalizeBlock(p).id}`);
                    setShowBacklinks(false);
                  }}
                  title={day || undefined}
                >
                  <span className="journal-page-link-icon text-[14px] flex-shrink-0">
                    {day ? "📅" : "📄"}
                  </span>
                  <span className="journal-page-link-title overflow-hidden text-ellipsis whitespace-nowrap">
                    {title || "Untitled"}
                  </span>
                </button>
              );
            })}
          </div>
        )}
      </aside>

      {/* ── Main content ─────────────────────────────────────────────── */}
      <main className="journal-main flex-1 overflow-y-auto p-[var(--space-6)] min-w-0">
        {showGraph ? (
          <div className="h-full w-full flex flex-col">
            <h2 className="text-[22px] font-bold text-[var(--text)] m-0 mb-[var(--space-4)]">
              Digital Garden Graph
            </h2>
            <div className="flex-1 min-h-0 border border-[var(--border)] rounded-[var(--radius-md)] bg-black/5 overflow-hidden">
              <GraphView onNodeClick={(id) => navigate(`page/${id}`)} />
            </div>
          </div>
        ) : isNewRoute ? (
          <NewBlockForm
            parentId={null}
            pageId={null}
            createBlock={(body) => createBlock.mutateAsync(body)}
            isPending={createBlock.isPending}
            onCreated={(data) => {
              queryClient.invalidateQueries({ queryKey: ["journal-pages"] });
              const newId = data?.n?.id || data?.id;
              if (newId) navigate(`page/${newId}`);
            }}
          />
        ) : activePage ? (
          <>
            <div className="journal-page-header mb-[var(--space-6)]">
              <h2 className="journal-page-title text-[22px] font-bold text-[var(--text)] m-0 mb-[var(--space-2)]">
                {fStr(activePage, "system:node_title") || "Untitled"}
              </h2>
              <div className="journal-page-meta flex items-center gap-[var(--space-3)] text-[13px] text-[var(--text-muted)] mb-[var(--space-2)]">
                {fStr(activePage, "journal:journal_day") && (
                  <span
                    data-testid="journal-date-badge"
                    className="journal-date-badge inline-flex items-center gap-[var(--space-1)] bg-[var(--badge-bg)] px-[var(--space-2)] py-[var(--space-1)] rounded-[var(--radius-sm)] text-xs"
                  >
                    📅 {fStr(activePage, "journal:journal_day")}
                  </span>
                )}
                <button
                  className={`${iconBtnSm} text-[var(--text-muted)] hover:text-[var(--text)]`}
                  onClick={() => setShowBacklinks(!showBacklinks)}
                >
                  🔗 Backlinks
                </button>
                <button
                  data-testid="journal-delete-btn"
                  className={`${iconBtnSm} text-[var(--danger)] hover:text-[var(--danger-hover)]`}
                  onClick={() => {
                    if (selectedPageId && confirm("Delete this page?"))
                      deleteBlock.mutate(selectedPageId, {
                        onSuccess: () => navigate("today"),
                      });
                  }}
                  title="Delete page"
                >
                  🗑️
                </button>
              </div>
              {fJson(activePage, "journal:tags") && (
                <div className="journal-tags flex flex-wrap gap-[var(--space-1)]">
                  {(Array.isArray(fJson(activePage, "journal:tags"))
                    ? fJson(activePage, "journal:tags")
                    : []
                  ).map((tag: string) => (
                    <span
                      key={tag}
                      className="journal-tag text-[11px] text-[var(--accent)] bg-[var(--accent-subtle)] px-[var(--space-2)] py-[var(--space-1)] rounded-[var(--radius-sm)]"
                    >
                      #{tag}
                    </span>
                  ))}
                </div>
              )}
            </div>

            {fJson(activePage, "journal:properties") && (
              <PropertiesBlock
                properties={fJson(activePage, "journal:properties")}
              />
            )}

            <BlockView
              block={activePage as BlockNode}
              depth={0}
              onEdit={(id) => setEditingBlockId(id)}
              editingBlockId={editingBlockId}
              onSave={(id, content) => updateBlock.mutate({ id, content })}
              onCancel={() => setEditingBlockId(null)}
              onDelete={(id) => {
                if (confirm("Delete this block?")) deleteBlock.mutate(id);
              }}
            />

            {pageChildren.map((child: BlockNode) => (
              <BlockView
                key={
                  normalizeBlock(child as any).id ||
                  fStr(child, "journal:order")
                }
                block={child}
                depth={1}
                onEdit={(id) => setEditingBlockId(id)}
                editingBlockId={editingBlockId}
                onSave={(id, content) => updateBlock.mutate({ id, content })}
                onCancel={() => setEditingBlockId(null)}
                onDelete={(id) => {
                  if (confirm("Delete this block?")) deleteBlock.mutate(id);
                }}
              />
            ))}

            <NewBlockForm
              parentId={selectedPageId}
              pageId={selectedPageId}
              createBlock={(body) => createBlock.mutateAsync(body)}
              isPending={createBlock.isPending}
              onCreated={() => {
                queryClient.invalidateQueries({
                  queryKey: ["journal-children"],
                });
                queryClient.invalidateQueries({ queryKey: ["journal-page"] });
              }}
            />

            {showBacklinks && (
              <div className="journal-backlinks mt-[var(--space-6)] p-[var(--space-4)] bg-[var(--bg-card)] border border-[var(--border)] rounded-[var(--radius-md)]">
                <h3 className="text-[14px] font-semibold text-[var(--text)] mb-[var(--space-3)]">
                  🔗 Backlinks
                </h3>
                {backlinks.length === 0 ? (
                  <p className="text-[var(--text-muted)] text-[13px]">
                    No backlinks yet. Link to this page with [[page name]].
                  </p>
                ) : (
                  backlinks.map((bl: BlockNode) => (
                    <div
                      key={normalizeBlock(bl as any).id}
                      className="journal-backlink-item p-[var(--space-3)] border border-[var(--border)] rounded-[var(--radius-sm)] mb-[var(--space-2)] bg-[var(--bg)]"
                    >
                      <div className="journal-backlink-title font-semibold text-[var(--text)] text-[13px] mb-[var(--space-1)]">
                        {fStr(bl, "system:node_title") || "Untitled"}
                      </div>
                      <div className="journal-backlink-preview text-[var(--text-muted)] text-[12px]">
                        {(fStr(bl, "journal:content") || "").slice(0, 200)}
                      </div>
                    </div>
                  ))
                )}
              </div>
            )}
          </>
        ) : (
          <div className="journal-empty flex items-center justify-center py-[var(--space-8)]">
            <p className="text-[var(--text-dim)]">
              Select a page or create a new one.
            </p>
          </div>
        )}
      </main>
    </div>
  );
}

// ── Block View ────────────────────────────────────────────────────────────────

export function BlockView({
  block,
  depth,
  onEdit,
  editingBlockId,
  onSave,
  onCancel,
  onDelete,
}: {
  block: BlockNode;
  depth: number;
  onEdit: (id: string) => void;
  editingBlockId: string | null;
  onSave: (id: string, content: string) => void;
  onCancel: () => void;
  onDelete: (id: string) => void;
}) {
  const id = normalizeBlock(block as any).id || "";
  const content = fStr(block, "journal:content") || "";
  const title = fStr(block, "system:node_title") || "";
  const isEditing = editingBlockId === id;
  const isDeleted = fBool(block, "journal:deleted");
  const [editContent, setEditContent] = useState(content);
  const [previewHtml, setPreviewHtml] = useState("");
  const [showPreview, setShowPreview] = useState(false);

  const renderContent = (text: string) =>
    text.replace(
      /\[\[([^\]]+)\]\]/g,
      '<span class="journal-ref text-[var(--accent)] bg-[var(--accent-subtle)] px-[var(--space-1)] rounded-[var(--radius-sm)]">$1</span>',
    );

  if (isDeleted) {
    return (
      <div className="journal-block deleted" style={{ marginLeft: depth * 28 }}>
        <span className="journal-bullet text-[var(--text-dim)] mr-[var(--space-2)]">
          ·
        </span>
        <span className="text-[var(--text-dim)] line-through">
          {title || content.slice(0, 80)}
        </span>
      </div>
    );
  }

  return (
    <div className="journal-block" style={{ marginLeft: depth * 28 }}>
      <div className="journal-block-row flex items-start gap-[var(--space-2)] group py-[var(--space-1)]">
        <span className="journal-bullet text-[var(--text-dim)] mt-[var(--space-1)] flex-shrink-0">
          {depth === 0 ? "◆" : "•"}
        </span>
        {isEditing ? (
          <div className="journal-block-editor flex-1">
            <TiptapEditor
              content={editContent}
              onChange={setEditContent}
              onSave={(html) => onSave(id, html)}
              onCancel={onCancel}
              autoFocus
            />
            <div className="journal-block-editor-actions flex gap-[var(--space-2)] mt-[var(--space-2)]">
              <button onClick={() => onSave(id, editContent)}>Save</button>
              <button onClick={onCancel}>Cancel</button>
              <button
                onClick={async () => {
                  const html = await renderMarkdown(editContent);
                  setPreviewHtml(html);
                  setShowPreview(!showPreview);
                }}
              >
                {showPreview ? "Edit" : "Preview"}
              </button>
            </div>
            {showPreview && (
              <div
                className="journal-markdown-preview mt-[var(--space-2)] p-[var(--space-3)] bg-[var(--bg)] border border-[var(--border)] rounded-[var(--radius-sm)] text-[14px]"
                dangerouslySetInnerHTML={{ __html: previewHtml }}
              />
            )}
          </div>
        ) : (
          <div
            className="journal-block-content flex-1 cursor-pointer min-h-[24px]"
            onClick={() => onEdit(id)}
          >
            <div
              className="journal-block-text text-[var(--text)] text-[15px] leading-relaxed"
              dangerouslySetInnerHTML={{
                __html:
                  renderContent(content) ||
                  `<span class="text-[var(--text-dim)]">${depth === 0 ? "Empty page — click to edit" : "Click to edit"}</span>`,
              }}
            />
          </div>
        )}
        {!isEditing && (
          <div className="journal-block-actions hidden group-hover:flex items-center gap-[var(--space-1)]">
            <button
              className={`journal-icon-btn-sm ${iconBtnSm}`}
              onClick={(e) => {
                e.stopPropagation();
                onEdit(id);
              }}
              title="Edit"
            >
              ✏️
            </button>
            <button
              className={`journal-icon-btn-sm ${iconBtnSm}`}
              onClick={(e) => {
                e.stopPropagation();
                onDelete(id);
              }}
              title="Delete"
            >
              🗑️
            </button>
          </div>
        )}
      </div>
    </div>
  );
}

// ── New Block Form ────────────────────────────────────────────────────────────

export function NewBlockForm({
  parentId,
  onCreated,
  createBlock,
  isPending,
}: {
  parentId: string | null;
  pageId: string | null;
  onCreated: (data: any) => void;
  createBlock: (body: any) => Promise<any>;
  isPending: boolean;
}) {
  const [title, setTitle] = useState("");
  const [content, setContent] = useState("");
  const [tags, setTags] = useState("");
  const [journalDay, setJournalDay] = useState(parentId === null ? TODAY : "");
  const isPage = parentId === null;

  // Accept optional HTML arg from TiptapEditor's onSave callback so we use the
  // editor's ground-truth content rather than React state, which may be stale
  // when Enter is pressed immediately after a programmatic fill() (e.g. in e2e).
  // Must guard — button onClick passes a React event, not an HTML string.
  const handleSubmit = async (htmlArg?: unknown) => {
    const submissionContent: string =
      typeof htmlArg === "string" ? htmlArg : content;
    if (!submissionContent.trim() && !title.trim()) return;
    const body: any = { content: submissionContent };
    if (title) body.title = title;
    if (parentId) body.parent_id = parentId;
    if (tags)
      body.tags = tags
        .split(",")
        .map((t) => t.trim())
        .filter(Boolean);
    if (journalDay) body.journal_day = journalDay;
    try {
      const data = await createBlock(body);
      setTitle("");
      setContent("");
      setTags("");
      onCreated(data);
    } catch (_e: any) {
      console.error("[NewBlockForm] createBlock failed:", _e?.message || _e, body);
    }
  };

  return (
    <div
      className="journal-new-block"
      style={{ marginLeft: parentId ? 28 : 0 }}
    >
      <div className="flex items-start gap-[var(--space-2)]">
        <span className="journal-bullet text-[var(--text-dim)] mt-[var(--space-2)]">
          {isPage ? "◆" : "•"}
        </span>
        <div className="journal-new-block-form flex-1 flex flex-col gap-[var(--space-2)]">
          {isPage && (
            <input
              type="text"
              placeholder="Page title"
              value={title}
              onChange={(e) => setTitle(e.target.value)}
              data-testid="journal-new-title-input"
              className="journal-new-title-input w-full bg-[var(--bg-input)] border border-[var(--border)] rounded-[var(--radius-sm)] px-[var(--space-3)] py-[var(--space-2)] text-[var(--text)] text-[14px]"
            />
          )}
          <TiptapEditor
            content={content}
            onChange={setContent}
            onSave={handleSubmit}
            minRows={3}
          />
          <div className="journal-new-block-meta flex gap-[var(--space-2)] items-center">
            <input
              type="text"
              placeholder="tags (comma-separated)"
              value={tags}
              onChange={(e) => setTags(e.target.value)}
              className="flex-1 text-[11px] px-[var(--space-2)] py-[var(--space-1)] bg-[var(--bg-input)] border border-[var(--border)] rounded-[var(--radius-sm)] text-[var(--text)]"
            />
            {isPage && (
              <input
                type="date"
                value={journalDay}
                onChange={(e) => setJournalDay(e.target.value)}
                className="text-[11px] px-[var(--space-2)] py-[var(--space-1)] bg-[var(--bg-input)] border border-[var(--border)] rounded-[var(--radius-sm)] text-[var(--text)]"
              />
            )}
            <button
              data-testid="journal-create-page-btn"
              onClick={handleSubmit}
              className="bg-[var(--accent)] border-[var(--accent)] text-[var(--accent-text)] text-[12px] px-[var(--space-3)] py-[var(--space-1)] rounded-[var(--radius-sm)] hover:bg-[var(--accent-hover)]"
            >
              {isPage ? "Create Page" : "Add Block"}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}

// ── Properties Block ──────────────────────────────────────────────────────────

export function PropertiesBlock({
  properties,
}: {
  properties: Record<string, any>;
}) {
  if (!properties || Object.keys(properties).length === 0) return null;
  return (
    <div className="journal-properties mb-[var(--space-4)] p-[var(--space-3)] bg-[var(--bg-card)] border border-[var(--border)] rounded-[var(--radius-md)]">
      {Object.entries(properties).map(([key, value]) => (
        <div
          key={key}
          className="journal-property-row flex gap-[var(--space-3)] py-[var(--space-1)] text-[13px]"
        >
          <span className="journal-property-key font-semibold text-[var(--text-muted)] min-w-[80px]">
            {key}
          </span>
          <span className="journal-property-value text-[var(--text)]">
            {typeof value === "boolean" ? (value ? "✅" : "⬜") : String(value)}
          </span>
        </div>
      ))}
    </div>
  );
}
