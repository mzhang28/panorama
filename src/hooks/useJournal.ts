import { useState, useEffect } from "react";

export interface JournalEntry {
  id: string;
  date: string;
  title: string;
  content: string;
  createdAt: Date;
  updatedAt: Date;
}

export function useJournal() {
  const [entries, setEntries] = useState<JournalEntry[]>([]);
  const [currentDate, setCurrentDate] = useState(() => {
    return new Date().toISOString().split("T")[0];
  });

  // Get today's journal entry
  const getTodayEntry = () => {
    const today = new Date().toISOString().split("T")[0];
    return entries.find((entry) => entry.date === today);
  };

  // Get entry by date
  const getEntryByDate = (date: string) => {
    return entries.find((entry) => entry.date === date);
  };

  // Create or update journal entry
  const updateEntry = (date: string, content: string) => {
    setEntries((prev) => {
      const existingIndex = prev.findIndex((entry) => entry.date === date);
      const now = new Date();

      if (existingIndex >= 0) {
        // Update existing entry
        const updated = [...prev];
        const existingEntry = updated[existingIndex];
        if (existingEntry) {
          updated[existingIndex] = {
            id: existingEntry.id,
            date: existingEntry.date,
            title: existingEntry.title,
            content,
            createdAt: existingEntry.createdAt,
            updatedAt: now,
          };
        }
        return updated;
      } else {
        // Create new entry
        const newEntry: JournalEntry = {
          id: `journal-${date}-${Date.now()}`,
          date,
          title: `Daily Journal - ${formatDate(date)}`,
          content,
          createdAt: now,
          updatedAt: now,
        };
        return [...prev, newEntry];
      }
    });
  };

  // Get recent entries (last 7 days)
  const getRecentEntries = () => {
    const sevenDaysAgo = new Date();
    sevenDaysAgo.setDate(sevenDaysAgo.getDate() - 7);

    return entries
      .filter((entry) => new Date(entry.date) >= sevenDaysAgo)
      .sort((a, b) => new Date(b.date).getTime() - new Date(a.date).getTime());
  };

  // Format date for display
  const formatDate = (dateString: string) => {
    const date = new Date(dateString);
    return date.toLocaleDateString("en-US", {
      weekday: "long",
      year: "numeric",
      month: "long",
      day: "numeric",
    });
  };

  // Initialize with today's entry if it doesn't exist
  useEffect(() => {
    const today = new Date().toISOString().split("T")[0];
    const todayEntry = entries.find((entry) => entry.date === today);
    if (!todayEntry && today) {
      const date = new Date(today);
      const formattedDate = date.toLocaleDateString("en-US", {
        weekday: "long",
        year: "numeric",
        month: "long",
        day: "numeric",
      });
      setEntries((prev) => {
        const newEntry: JournalEntry = {
          id: `journal-${today}-${Date.now()}`,
          date: today,
          title: `Daily Journal - ${formattedDate}`,
          content: "",
          createdAt: new Date(),
          updatedAt: new Date(),
        };
        return [...prev, newEntry];
      });
    }
  }, [entries.length]);

  return {
    entries,
    currentDate,
    setCurrentDate,
    getTodayEntry,
    getEntryByDate,
    updateEntry,
    getRecentEntries,
    formatDate,
  };
}
