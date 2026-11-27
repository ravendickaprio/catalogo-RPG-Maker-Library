import { maintenanceList } from "../api/client";
import type { CheckId, Issue } from "../api/types";

const PAGE = 500;

/** Pages `maintenance_list` (500 per page) until every issue of the detector has been collected. */
export async function loadAllIssueIds(check: CheckId): Promise<Issue[]> {
  const all: Issue[] = [];
  let off = 0;
  for (;;) {
    const p = await maintenanceList(check, off, PAGE);
    all.push(...p.items);
    off += p.items.length;
    if (!p.items.length || off >= p.total) break;
  }
  return all;
}
