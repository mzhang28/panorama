import FullCalendar from "@fullcalendar/react";
import dayGridPlugin from "@fullcalendar/daygrid";
import timeGridPlugin from "@fullcalendar/timegrid";
import listPlugin from "@fullcalendar/list";
import bootstrap5Plugin from "@fullcalendar/bootstrap5";
import { useCallback, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import type { QueryEventsResponse } from "../../backend/bindings/QueryEventsResponse";

export default function Calendar() {
  const [startDate, setStartDate] = useState<Date | null>(null);
  const [endDate, setEndDate] = useState<Date | null>(null);

  const fetchEvents = useCallback(async () => {
    if (startDate === null || endDate === null) return null;
    const params = new URLSearchParams();
    params.set("start_date", startDate.toISOString());
    params.set("end_date", endDate.toISOString());
    console.log("params", params.toString());
    const res = await fetch(`/api/apps/cal/events?${params.toString()}`);
    const data: QueryEventsResponse = await res.json();
    return data.events.map((event) => ({
      title: event.title,
      date: event.start_date,
    }));
  }, [startDate, endDate]);

  const { data: events } = useQuery({
    queryKey: ["cal/events", startDate, endDate],
    queryFn: fetchEvents,
  });

  return (
    <div className="p-3 grow flex flex-col">
      <FullCalendar
        plugins={[dayGridPlugin, timeGridPlugin, listPlugin, bootstrap5Plugin]}
        themeSystem="bootstrap5"
        initialView="dayGridMonth"
        height="100%"
        expandRows={true}
        datesSet={(dateInfo) => {
          setStartDate(dateInfo.start);
          setEndDate(dateInfo.end);
          console.log("set date info", dateInfo);
        }}
        headerToolbar={{
          right: "dayGridMonth,timeGridWeek,listWeek prev,today,next",
        }}
        events={events ?? []}
      />
    </div>
  );
}
