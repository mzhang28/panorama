import FullCalendar from "@fullcalendar/react";
import dayGridPlugin from "@fullcalendar/daygrid";
import bootstrap5Plugin from "@fullcalendar/bootstrap5";
import { useCallback, useState } from "react";
import { useQuery } from "@tanstack/react-query";

export default function Calendar() {
  const [startDate, setStartDate] = useState<Date | null>(null);
  const [endDate, setEndDate] = useState<Date | null>(null);

  const fetchEvents = useCallback(async () => {
    if (startDate === null || endDate === null) return null;
    const res = await fetch("/api/apps/cal/events");
    const data = await res.json();
    return data.events.map((event) => ({
      title: event.title,
      date: event.start_date,
    }));
  }, [startDate, endDate]);

  const { data: events } = useQuery({
    queryKey: ["cal/events"],
    queryFn: fetchEvents,
  });

  return (
    <FullCalendar
      plugins={[dayGridPlugin, bootstrap5Plugin]}
      themeSystem="bootstrap5"
      initialView="dayGridMonth"
      datesSet={(dateInfo) => {
        setStartDate(dateInfo.start);
        setEndDate(dateInfo.end);
        console.log("set date info", dateInfo);
      }}
      events={events ?? []}
    />
  );
}
