import { atom, useAtomValue } from "jotai";
import { useCallback, useMemo } from "react";

export const homeserverUrlAtom = atom("http://minihost:10020");

export function useFetchApi(): <T>(
  _: string,
  init?: RequestInit,
) => Promise<T> {
  const homeserverUrl = useAtomValue(homeserverUrlAtom);

  return useMemo(
    () =>
      async <T>(path: string, init?: RequestInit) => {
        const homeserverUrl2 = homeserverUrl.replace(/\/$/, "");
        const path2 = path.replace(/^\//, "");
        const url = `${homeserverUrl2}/api/${path2}`;
        console.log("requesting", url, init);
        const res = await fetch(url, init);
        const data: T = await res.json();
        return data;
      },
    [homeserverUrl],
  );
}

export async function fetchApi<T>(base: string): Promise<T> {
  const res = await fetch(`${base}`);
  const data: T = await res.json();
  return data;
}
