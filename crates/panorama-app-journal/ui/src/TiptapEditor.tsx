import { useEditor, EditorContent } from '@tiptap/react';
import StarterKit from '@tiptap/starter-kit';
import Placeholder from '@tiptap/extension-placeholder';
import { useEffect } from 'react';

export function TiptapEditor({
  content,
  onChange,
  onSave,
  onCancel,
  autoFocus,
  minRows = 2,
}: {
  content: string;
  onChange?: (html: string) => void;
  onSave?: (html: string) => void;
  onCancel?: () => void;
  autoFocus?: boolean;
  minRows?: number;
}) {
  const editor = useEditor({
    extensions: [
      StarterKit.configure({
        heading: {
          levels: [1, 2, 3],
        }
      }),
      Placeholder.configure({
        placeholder: 'Write something, or type "/" for commands...',
      }),
    ],
    content,
    editorProps: {
      attributes: {
        class: `tiptap-editor-prose focus:outline-none text-[var(--text)]`,
        style: `min-height: ${minRows * 24}px;`,
      },
    },
    onUpdate: ({ editor }) => {
      if (onChange) {
        onChange(editor.getHTML());
      }
    },
  });

  useEffect(() => {
    if (editor && !editor.isDestroyed && content !== editor.getHTML()) {
      editor.commands.setContent(content);
    }
  }, [content, editor]);

  useEffect(() => {
    if (editor && !editor.isDestroyed && autoFocus) {
      editor.commands.focus('end');
    }
  }, [editor, autoFocus]);

  return (
    <div 
      className="tiptap-wrapper w-full bg-[var(--bg-input)] border border-[var(--border)] rounded-[var(--radius-sm)] px-[var(--space-3)] py-[var(--space-2)] text-[14px]"
      onKeyDown={(e) => {
        if (e.key === 'Enter' && !e.shiftKey) {
          if (onSave && editor && !editor.isDestroyed) {
            e.preventDefault();
            onSave(editor.getHTML());
          }
        }
        if (e.key === 'Escape') {
          if (onCancel) {
            e.preventDefault();
            onCancel();
          }
        }
      }}
    >
      <EditorContent editor={editor} />
    </div>
  );
}
