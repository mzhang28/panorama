import {
  QueryKey,
  UndefinedInitialDataOptions,
  useQuery,
} from "@tanstack/react-query";
import { atom, useAtomValue } from "jotai";
import { useCallback, useMemo } from "react";

// export const homeserverUrlAtom = atom("http://10.0.0.234:5173");
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
        console.log("result", data);
        return data;
      },
    [homeserverUrl],
  );
}

export function useApiQuery<
  TQueryFnData = unknown,
  TError = Error,
  TData = TQueryFnData,
>(options: UndefinedInitialDataOptions<TQueryFnData, TError, TData>) {
  const homeserverUrl = useAtomValue(homeserverUrlAtom);
  return useQuery({
    ...options,
    queryKey: [homeserverUrl, ...options.queryKey],
  });
}
