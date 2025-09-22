import { useState } from "react";

export interface Todo {
  id: string;
  text: string;
  completed: boolean;
  createdAt: Date;
  completedAt?: Date;
}

export function useTodos() {
  const [todos, setTodos] = useState<Todo[]>([
    {
      id: "1",
      text: "Review project documentation",
      completed: false,
      createdAt: new Date(),
    },
    {
      id: "2",
      text: "Plan weekly goals",
      completed: false,
      createdAt: new Date(),
    },
    {
      id: "3",
      text: "Update daily notes",
      completed: true,
      createdAt: new Date(),
      completedAt: new Date(),
    },
  ]);

  const addTodo = (text: string) => {
    const newTodo: Todo = {
      id: `todo-${Date.now()}`,
      text,
      completed: false,
      createdAt: new Date(),
    };
    setTodos((prev) => [newTodo, ...prev]);
  };

  const toggleTodo = (id: string) => {
    setTodos((prev) =>
      prev.map((todo) => {
        if (todo.id === id) {
          return {
            ...todo,
            completed: !todo.completed,
            completedAt: !todo.completed ? new Date() : undefined,
          };
        }
        return todo;
      }),
    );
  };

  const deleteTodo = (id: string) => {
    setTodos((prev) => prev.filter((todo) => todo.id !== id));
  };

  const updateTodo = (id: string, text: string) => {
    setTodos((prev) =>
      prev.map((todo) => (todo.id === id ? { ...todo, text } : todo)),
    );
  };

  const getActiveTodos = () => {
    return todos.filter((todo) => !todo.completed);
  };

  const getCompletedTodos = () => {
    return todos.filter((todo) => todo.completed);
  };

  return {
    todos,
    addTodo,
    toggleTodo,
    deleteTodo,
    updateTodo,
    getActiveTodos,
    getCompletedTodos,
  };
}
