import React, { useState } from "react";
import {
  Calendar as CalendarIcon,
  Plus,
  Check,
  X,
  ChevronLeft,
  ChevronRight,
} from "lucide-react";
import { useTodos } from "../hooks/useTodos";
import { useJournal } from "../hooks/useJournal";

export function RightSidebar() {
  const {
    todos,
    addTodo,
    toggleTodo,
    deleteTodo,
    getActiveTodos,
    getCompletedTodos,
  } = useTodos();
  const { setCurrentDate, currentDate } = useJournal();
  const [newTodoText, setNewTodoText] = useState("");
  const [showCompleted, setShowCompleted] = useState(false);
  const [currentMonth, setCurrentMonth] = useState(new Date());

  const activeTodos = getActiveTodos();
  const completedTodos = getCompletedTodos();

  const handleAddTodo = () => {
    if (newTodoText.trim()) {
      addTodo(newTodoText.trim());
      setNewTodoText("");
    }
  };

  const handleKeyPress = (e: React.KeyboardEvent) => {
    if (e.key === "Enter") {
      handleAddTodo();
    }
  };

  // Calendar functions
  const getDaysInMonth = (date: Date) => {
    return new Date(date.getFullYear(), date.getMonth() + 1, 0).getDate();
  };

  const getFirstDayOfMonth = (date: Date) => {
    return new Date(date.getFullYear(), date.getMonth(), 1).getDay();
  };

  const handleDateClick = (day: number) => {
    const selectedDate = new Date(
      currentMonth.getFullYear(),
      currentMonth.getMonth(),
      day,
    );
    const dateString = selectedDate.toISOString().split("T")[0];
    setCurrentDate(dateString);
  };

  const isToday = (day: number) => {
    const today = new Date();
    return (
      today.getDate() === day &&
      today.getMonth() === currentMonth.getMonth() &&
      today.getFullYear() === currentMonth.getFullYear()
    );
  };

  const isSelected = (day: number) => {
    if (!currentDate) return false;
    const selected = new Date(currentDate);
    return (
      selected.getDate() === day &&
      selected.getMonth() === currentMonth.getMonth() &&
      selected.getFullYear() === currentMonth.getFullYear()
    );
  };

  const goToPreviousMonth = () => {
    setCurrentMonth(
      new Date(currentMonth.getFullYear(), currentMonth.getMonth() - 1),
    );
  };

  const goToNextMonth = () => {
    setCurrentMonth(
      new Date(currentMonth.getFullYear(), currentMonth.getMonth() + 1),
    );
  };

  const monthNames = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
  ];

  const dayNames = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];

  const renderCalendar = () => {
    const daysInMonth = getDaysInMonth(currentMonth);
    const firstDay = getFirstDayOfMonth(currentMonth);
    const days = [];

    // Add empty cells for days before the first day of the month
    for (let i = 0; i < firstDay; i++) {
      days.push(
        <div
          key={`empty-${i}`}
          className="h-8 flex items-center justify-center"
        ></div>,
      );
    }

    // Add days of the month
    for (let day = 1; day <= daysInMonth; day++) {
      days.push(
        <button
          key={day}
          onClick={() => handleDateClick(day)}
          className={`h-8 flex items-center justify-center text-sm rounded hover:bg-gray-100 transition-colors ${
            isToday(day)
              ? "bg-blue-500 text-white hover:bg-blue-600"
              : isSelected(day)
                ? "bg-blue-100 text-blue-700 border border-blue-300"
                : "text-gray-700"
          }`}
        >
          {day}
        </button>,
      );
    }

    return days;
  };

  return (
    <div className="h-full flex flex-col bg-white">
      {/* Calendar Section */}
      <div className="p-4 border-b border-gray-100">
        <div className="flex items-center justify-between mb-4">
          <h2 className="text-sm font-semibold text-gray-700 uppercase tracking-wider flex items-center gap-2">
            <CalendarIcon size={16} />
            Calendar
          </h2>
        </div>

        {/* Calendar Header */}
        <div className="flex items-center justify-between mb-3">
          <button
            onClick={goToPreviousMonth}
            className="p-1 rounded hover:bg-gray-100 transition-colors"
          >
            <ChevronLeft size={16} className="text-gray-600" />
          </button>

          <h3 className="text-sm font-medium text-gray-900">
            {monthNames[currentMonth.getMonth()]} {currentMonth.getFullYear()}
          </h3>

          <button
            onClick={goToNextMonth}
            className="p-1 rounded hover:bg-gray-100 transition-colors"
          >
            <ChevronRight size={16} className="text-gray-600" />
          </button>
        </div>

        {/* Calendar Grid */}
        <div className="grid grid-cols-7 gap-1">
          {/* Day headers */}
          {dayNames.map((day) => (
            <div
              key={day}
              className="h-6 flex items-center justify-center text-xs text-gray-500 font-medium"
            >
              {day}
            </div>
          ))}

          {/* Calendar days */}
          {renderCalendar()}
        </div>
      </div>

      {/* Todos Section */}
      <div className="flex-1 flex flex-col">
        <div className="p-4 border-b border-gray-100">
          <div className="flex items-center justify-between mb-3">
            <h2 className="text-sm font-semibold text-gray-700 uppercase tracking-wider">
              Tasks
            </h2>
            <span className="text-xs text-gray-500 bg-gray-100 px-2 py-1 rounded-full">
              {activeTodos.length} active
            </span>
          </div>

          {/* Add new todo */}
          <div className="flex gap-2 mb-4">
            <input
              type="text"
              value={newTodoText}
              onChange={(e) => setNewTodoText(e.currentTarget.value)}
              onKeyPress={handleKeyPress}
              placeholder="Add a new task..."
              className="flex-1 px-3 py-2 text-sm border border-gray-200 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent"
            />
            <button
              onClick={handleAddTodo}
              disabled={!newTodoText.trim()}
              className="p-2 bg-blue-600 text-white rounded-md hover:bg-blue-700 disabled:bg-gray-300 disabled:cursor-not-allowed transition-colors"
            >
              <Plus size={14} />
            </button>
          </div>
        </div>

        {/* Todo List */}
        <div className="flex-1 overflow-y-auto p-4">
          {/* Active Todos */}
          <div className="space-y-2 mb-6">
            {activeTodos.map((todo) => (
              <div
                key={todo.id}
                className="flex items-center gap-3 p-3 bg-gray-50 rounded-lg border border-gray-100 group"
              >
                <button
                  onClick={() => toggleTodo(todo.id)}
                  className="w-4 h-4 border-2 border-gray-300 rounded flex items-center justify-center hover:border-blue-500 transition-colors"
                >
                  <Check size={10} className="text-transparent" />
                </button>

                <span className="flex-1 text-sm text-gray-900">
                  {todo.text}
                </span>

                <button
                  onClick={() => deleteTodo(todo.id)}
                  className="opacity-0 group-hover:opacity-100 p-1 text-gray-400 hover:text-red-500 transition-all"
                >
                  <X size={14} />
                </button>
              </div>
            ))}

            {activeTodos.length === 0 && (
              <div className="text-center py-6">
                <div className="w-12 h-12 bg-gray-100 rounded-full flex items-center justify-center mx-auto mb-2">
                  <Check size={20} className="text-gray-400" />
                </div>
                <p className="text-sm text-gray-500">All tasks completed!</p>
              </div>
            )}
          </div>

          {/* Completed Todos */}
          {completedTodos.length > 0 && (
            <div>
              <button
                onClick={() => setShowCompleted(!showCompleted)}
                className="flex items-center gap-2 text-sm text-gray-600 hover:text-gray-900 transition-colors mb-3"
              >
                <ChevronRight
                  size={14}
                  className={`transition-transform ${showCompleted ? "rotate-90" : ""}`}
                />
                <span>Completed ({completedTodos.length})</span>
              </button>

              {showCompleted && (
                <div className="space-y-2">
                  {completedTodos.map((todo) => (
                    <div
                      key={todo.id}
                      className="flex items-center gap-3 p-3 bg-green-50 rounded-lg border border-green-100 group"
                    >
                      <button
                        onClick={() => toggleTodo(todo.id)}
                        className="w-4 h-4 border-2 border-green-500 bg-green-500 rounded flex items-center justify-center hover:bg-green-600 transition-colors"
                      >
                        <Check size={10} className="text-white" />
                      </button>

                      <span className="flex-1 text-sm text-gray-600 line-through">
                        {todo.text}
                      </span>

                      <button
                        onClick={() => deleteTodo(todo.id)}
                        className="opacity-0 group-hover:opacity-100 p-1 text-gray-400 hover:text-red-500 transition-all"
                      >
                        <X size={14} />
                      </button>
                    </div>
                  ))}
                </div>
              )}
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
