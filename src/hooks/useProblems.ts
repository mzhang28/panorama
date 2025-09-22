import { useState, useCallback } from "react";

export interface ProblemStatus {
  errors: number;
  warnings: number;
  info: number;
}

export interface Problem {
  id: string;
  type: "error" | "warning" | "info";
  message: string;
  file?: string;
  line?: number;
  timestamp: Date;
}

interface UseProblemsReturn {
  problems: ProblemStatus;
  allProblems: Problem[];
  addProblem: (problem: Omit<Problem, "id" | "timestamp">) => void;
  removeProblem: (id: string) => void;
  clearProblems: (type?: Problem["type"]) => void;
  updateProblems: (newProblems: Problem[]) => void;
}

export function useProblems(): UseProblemsReturn {
  const [allProblems, setAllProblems] = useState<Problem[]>([]);

  const calculateProblemCounts = useCallback((problems: Problem[]): ProblemStatus => {
    return problems.reduce(
      (acc, problem) => {
        acc[problem.type === "error" ? "errors" : problem.type === "warning" ? "warnings" : "info"]++;
        return acc;
      },
      { errors: 0, warnings: 0, info: 0 }
    );
  }, []);

  const problems = calculateProblemCounts(allProblems);

  const addProblem = useCallback((problemData: Omit<Problem, "id" | "timestamp">) => {
    const newProblem: Problem = {
      ...problemData,
      id: `${Date.now()}-${Math.random().toString(36).substr(2, 9)}`,
      timestamp: new Date(),
    };

    setAllProblems(prev => [...prev, newProblem]);
  }, []);

  const removeProblem = useCallback((id: string) => {
    setAllProblems(prev => prev.filter(problem => problem.id !== id));
  }, []);

  const clearProblems = useCallback((type?: Problem["type"]) => {
    if (type) {
      setAllProblems(prev => prev.filter(problem => problem.type !== type));
    } else {
      setAllProblems([]);
    }
  }, []);

  const updateProblems = useCallback((newProblems: Problem[]) => {
    setAllProblems(newProblems);
  }, []);

  return {
    problems,
    allProblems,
    addProblem,
    removeProblem,
    clearProblems,
    updateProblems,
  };
}
