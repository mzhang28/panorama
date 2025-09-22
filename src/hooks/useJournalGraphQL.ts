import { useState, useEffect, useCallback } from "react";
import { trpc } from "../lib/trpc";

export interface JournalEntry {
  id: string;
  date: string;
  title: string;
  content: string;
  createdAt: Date;
  updatedAt: Date;
}

export function useJournalGraphQL() {
  const [entries, setEntries] = useState<JournalEntry[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<Error | null>(null);

  // Fetch all journal entries
  const fetchEntries = useCallback(async () => {
    try {
      setIsLoading(true);
      setError(null);

      const result = (await trpc.graphql.query({
        query: `
          query GetJournalEntries {
            nodes(type: "JournalEntry") {
              id
              extraFields
            }
          }
        `,
      })) as any;

      console.log("RESULT:", result);

      if (result.errors) {
        throw new Error(result.errors[0].message);
      }

      const nodes = result.data?.nodes || [];
      const journalEntries: JournalEntry[] = nodes.map((node: any) => {
        const extraFields = node.extraFields ? JSON.parse(node.extraFields) : {};
        return {
          id: node.id,
          date: extraFields.date || "",
          title: extraFields.title || "",
          content: extraFields.content || "",
          createdAt: new Date(extraFields.createdAt || node.created_at),
          updatedAt: new Date(extraFields.updatedAt || node.updated_at),
        };
      });

      setEntries(journalEntries);
    } catch (err) {
      const error =
        err instanceof Error
          ? err
          : new Error("Failed to fetch journal entries");
      setError(error);
      console.error("Failed to fetch journal entries:", error);

      // Retry if database is not ready
      if (error.message?.includes("Database not ready")) {
        setTimeout(() => fetchEntries(), 1000);
      }
    } finally {
      setIsLoading(false);
    }
  }, []);

  // Create or update journal entry
  const saveEntry = useCallback(
    async (entry: JournalEntry) => {
      try {
        const extraFields = {
          date: entry.date,
          title: entry.title,
          content: entry.content,
          createdAt: entry.createdAt.toISOString(),
          updatedAt: entry.updatedAt.toISOString(),
        };

        const result = (await trpc.graphql.query({
          query: `
          mutation SaveJournalEntry($data: String!) {
            createNode(type: "JournalEntry", data: $data)
          }
        `,
          variables: {
            data: JSON.stringify(extraFields),
          },
        })) as any;

        if (result.errors) {
          throw new Error(result.errors[0].message);
        }

        // Refresh entries after save
        await fetchEntries();

        return result.data?.createNode;
      } catch (err) {
        const error =
          err instanceof Error
            ? err
            : new Error("Failed to save journal entry");
        setError(error);
        console.error("Failed to save journal entry:", error);
        throw error;
      }
    },
    [fetchEntries],
  );

  // Get today's journal entry
  const getTodayEntry = useCallback(() => {
    const today = new Date().toISOString().split("T")[0];
    return entries.find((entry) => entry.date === today);
  }, [entries]);

  // Get entry by date
  const getEntryByDate = useCallback(
    (date: string) => {
      return entries.find((entry) => entry.date === date);
    },
    [entries],
  );

  // Create or update journal entry
  const updateEntry = useCallback(
    (date: string, content: string) => {
      const existingIndex = entries.findIndex((entry) => entry.date === date);
      const now = new Date();

      if (existingIndex >= 0) {
        // Update existing entry
        const existingEntry = entries[existingIndex];
        if (!existingEntry) return;

        const updatedEntry: JournalEntry = {
          id: existingEntry.id,
          date: existingEntry.date,
          title: existingEntry.title,
          content,
          createdAt: existingEntry.createdAt,
          updatedAt: now,
        };
        setEntries((prev) => {
          const updated = [...prev];
          updated[existingIndex] = updatedEntry;
          return updated;
        });
        // Save to database
        saveEntry(updatedEntry);
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
        setEntries((prev) => [...prev, newEntry]);
        // Save to database
        saveEntry(newEntry);
      }
    },
    [entries, saveEntry],
  );

  // Get recent entries (last 7 days)
  const getRecentEntries = useCallback(() => {
    const sevenDaysAgo = new Date();
    sevenDaysAgo.setDate(sevenDaysAgo.getDate() - 7);

    return entries
      .filter((entry) => new Date(entry.date) >= sevenDaysAgo)
      .sort((a, b) => new Date(b.date).getTime() - new Date(a.date).getTime());
  }, [entries]);

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
    if (!todayEntry && today && !isLoading) {
      const date = new Date(today);
      const formattedDate = date.toLocaleDateString("en-US", {
        weekday: "long",
        year: "numeric",
        month: "long",
        day: "numeric",
      });
      const newEntry: JournalEntry = {
        id: `journal-${today}-${Date.now()}`,
        date: today,
        title: `Daily Journal - ${formattedDate}`,
        content: "",
        createdAt: new Date(),
        updatedAt: new Date(),
      };
      setEntries((prev) => [...prev, newEntry]);
    }
  }, [entries, isLoading]);

  // Load entries on mount
  useEffect(() => {
    fetchEntries();
  }, [fetchEntries]);

  return {
    entries,
    isLoading,
    error,
    getTodayEntry,
    getEntryByDate,
    updateEntry,
    getRecentEntries,
    formatDate,
    refetch: fetchEntries,
  };
}
