import { useEffect } from 'react';
import { LexicalComposer } from '@lexical/react/LexicalComposer';
import { PlainTextPlugin } from '@lexical/react/LexicalPlainTextPlugin';
import { ContentEditable } from '@lexical/react/LexicalContentEditable';
import { HistoryPlugin } from '@lexical/react/LexicalHistoryPlugin';
import { OnChangePlugin } from '@lexical/react/LexicalOnChangePlugin';
import { useLexicalComposerContext } from '@lexical/react/LexicalComposerContext';
import { LexicalErrorBoundary } from '@lexical/react/LexicalErrorBoundary';
import { $getRoot, $createTextNode, $createParagraphNode, FOCUS_COMMAND } from 'lexical';
import type { EditorState } from 'lexical';

interface JournalEditorProps {
  value: string;
  onChange: (value: string) => void;
  onFocus?: () => void;
  placeholder?: string;
  className?: string;
}

function EditorContent({ onChange }: { onChange: (value: string) => void }) {
  return (
    <OnChangePlugin
      onChange={(editorState: EditorState) => {
        editorState.read(() => {
          const root = $getRoot();
          const textContent = root.getTextContent();
          onChange(textContent);
        });
      }}
    />
  );
}



function LoadContentPlugin({ content }: { content: string }) {
  const [editor] = useLexicalComposerContext();

  useEffect(() => {
    if (content) {
      editor.update(() => {
        const root = $getRoot();
        root.clear();
        const paragraph = $createParagraphNode();
        const textNode = $createTextNode(content);
        paragraph.append(textNode);
        root.append(paragraph);
      });
    }
  }, [content, editor]);

  return null;
}

export function JournalEditor({ value, onChange, onFocus, placeholder, className }: JournalEditorProps) {
  const initialConfig = {
    namespace: 'JournalEditor',
    theme: {
      text: {
        bold: 'font-bold',
        italic: 'italic',
        underline: 'underline',
      },
    },
    onError: (error: Error) => {
      console.error('Lexical error:', error);
    },
  };

  return (
    <div className={className}>
      <LexicalComposer initialConfig={initialConfig}>
        <PlainTextPlugin
          contentEditable={
            <ContentEditable
              onFocus={onFocus}
              className="min-h-[24rem] p-6 border border-gray-200 rounded-lg resize-none focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent text-gray-900 leading-relaxed"
              style={{
                fontSize: '16px',
                lineHeight: '1.6',
                fontFamily: '-apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif',
              }}
            />
          }
          placeholder={
            <div className="absolute top-6 left-6 text-gray-400 pointer-events-none">
              {placeholder}
            </div>
          }
          ErrorBoundary={LexicalErrorBoundary}
        />
        <HistoryPlugin />
        <EditorContent onChange={onChange} />
        <LoadContentPlugin content={value} />
      </LexicalComposer>
    </div>
  );
}