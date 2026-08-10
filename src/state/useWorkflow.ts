// SPDX-License-Identifier: GPL-3.0-or-later
import { useContext } from "react";
import { WorkflowContext, type WorkflowContextValue } from "./workflowContextInstance";

export function useWorkflow(): WorkflowContextValue {
  const ctx = useContext(WorkflowContext);
  if (!ctx) {
    throw new Error("useWorkflow must be used within a WorkflowProvider");
  }
  return ctx;
}
