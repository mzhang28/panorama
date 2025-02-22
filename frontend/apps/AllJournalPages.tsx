import { format } from "date-fns";
import { useEffect } from "react";
import { useCallback } from "react";
import { useState } from "react";
import { useRef } from "react";
import JournalPage from "./JournalPage";
import type { GetJournalResponse } from "../../backend/bindings/GetJournalResponse";

const formatDate = (date: Date) => format(date, "yyyy-MM-dd");

export default function AllJournalPages() {
  const [todaysDate, setTodaysDate] = useState<string>(() =>
    formatDate(new Date())
  );
  const [journalPages, setJournalPages] = useState<string[]>([]);
  const endDetector = useRef<HTMLDivElement | null>(null);
  const [reobserve, setReobserve] = useState(0);

  useEffect(() => {
    const func = () => {
      const oldDate = todaysDate;
      const newDate = formatDate(new Date());
      if (newDate !== oldDate) {
        setTodaysDate(newDate);
        setJournalPages([]);
        setReobserve((c) => c + 1);
      }
    };

    func();
    const interval = setInterval(func, 10 * 60 * 1000);

    return () => {
      clearInterval(interval);
    };
  }, [todaysDate]);

  useEffect(() => {
    const observer = new IntersectionObserver((entries) => {
      for (const entry of entries) {
        if (entry.isIntersecting) {
          console.log("INTERSECTING");

          // Get the next journal page
          if (journalPages.length === 0) {
            setJournalPages([...journalPages, todaysDate]);
            setReobserve((c) => c + 1);
          } else {
            (async () => {
              const lastDate = journalPages[journalPages.length - 1];
              const res = await fetch(
                `/api/apps/journal/by_date/${lastDate}/prev`
              );
              const data: GetJournalResponse | null = await res.json();
              if (data === null) {
                // This is the last page
              } else {
                setJournalPages([...journalPages, data.date]);
                setReobserve((c) => c + 1);
              }
            })();
          }
        }
      }
    });

    if (endDetector.current) observer.observe(endDetector.current);

    return () => {
      if (endDetector.current) observer.unobserve(endDetector.current);
    };
  }, [reobserve, todaysDate, journalPages]);

  return (
    <div className="flex flex-col gap-3 grow">
      {journalPages.map((date) => (
        <JournalPage date={date} key={date} />
      ))}

      <div className="text-slate-500 text-center" ref={endDetector}>
        <i>fin</i>
      </div>
    </div>
  );
}
