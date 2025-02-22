import { Copy, Check } from "lucide-react";
import { useCallback } from "react";
import { useState } from "react";

export interface CopyButtonProps {
  contents: string;
  size?: number;
}

export default function CopyButton({ contents, size }: CopyButtonProps) {
  const [timeoutHandle, setTimeoutHandle] = useState<Timer | null>(null);
  const [copied, setCopied] = useState(false);

  const copy = useCallback(async () => {
    const blob = new Blob([contents], { type: "text/plain" });
    await navigator.clipboard.write([
      new ClipboardItem({ "text/plain": blob }),
    ]);

    if (timeoutHandle === null) {
      setCopied(true);
      const handle = setTimeout(() => {
        setCopied(false);
        setTimeoutHandle(null);
      }, 2000);
      setTimeoutHandle(handle);
    }
  }, [timeoutHandle]);

  return (
    <button className="p-1 hover:bg-slate-200" onClick={copy}>
      {copied ? <Check size={size ?? 12} /> : <Copy size={size ?? 12} />}
    </button>
  );
}
