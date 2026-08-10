// SPDX-License-Identifier: GPL-3.0-or-later
import { useContext } from "react";
import { AnnouncerContext, type Announce } from "./announcerContextInstance";

export function useAnnounce(): Announce {
  const ctx = useContext(AnnouncerContext);
  if (!ctx) {
    throw new Error("useAnnounce must be used within a LiveAnnouncerProvider");
  }
  return ctx;
}
